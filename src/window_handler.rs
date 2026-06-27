use super::SetupHandler;
use super::SlintEditorState;
use crate::window_adapter::BaseviewSlintAdapter;
use baseview::Event;
use baseview::EventStatus;
use baseview::WindowInfo;
use baseview::{Size, WindowEvent as BaseviewWindowEvent};
use i_slint_renderer_femtovg::FemtoVGRenderer;
use nice_plug_core::context::gui::GuiContext;
// use nice_plug_core::context::gui::ParamSetter;
use slint::platform::WindowEvent;
use slint::LogicalPosition;
use slint::SharedString;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// Per-window state, passed to the event loop handler each frame.
pub struct WindowHandler<T: slint::ComponentHandle> {
    // pub context: Arc<dyn GuiContext>,
    // pub(crate) event_loop_handler: Arc<EventLoopHandler<T>>,
    pub setup_handler: Arc<SetupHandler<T>>,
    pub scale_factor: RefCell<f32>,
    pub state: Arc<SlintEditorState>,
    // Rc so it can be shared with Slint callbacks without needing &mut self
    pub pending_resizes: Rc<RefCell<Vec<(u32, u32)>>>,
    pub last_cursor_pos: RefCell<LogicalPosition>,
    pub window_shown: RefCell<bool>,
    pub component: T,
    pub adapter: Rc<BaseviewSlintAdapter>,
    pub prevent_key_event_propagation: RefCell<bool>,
}

impl<T: slint::ComponentHandle> WindowHandler<T> {
    /// Resize the window. `width` and `height` are in logical pixels.
    pub fn resize(&self, window: &mut baseview::Window, width: u32, height: u32) {
        let scale = *self.scale_factor.borrow();
        let physical_width = (width as f32 * scale) as u32;
        let physical_height = (height as f32 * scale) as u32;

        self.state.size.store((width, height));

        // Update adapter with physical size and scale factor
        self.adapter
            .update_size(physical_width, physical_height, scale);

        // Notify Slint window of new size to trigger re-layout
        // Slint expects logical size here
        let slint_window = self.window();
        slint_window.dispatch_event(slint::platform::WindowEvent::Resized {
            size: slint::LogicalSize::new(width as f32, height as f32),
        });

        // Request redraw to show changes immediately
        slint_window.request_redraw();

        // Notify host
        // self.context.request_resize();

        // Resize baseview window (uses logical size, baseview handles physical conversion)
        window.resize(Size {
            width: width as f64,
            height: height as f64,
        });
    }

    /// Handle a window info update from baseview (scale factor or size change)
    pub(crate) fn handle_window_info(&self, info: &WindowInfo) {
        let scale = info.scale() as f32;
        let physical_size = info.physical_size();

        *self.scale_factor.borrow_mut() = scale;

        // Update adapter with physical size
        self.adapter
            .update_size(physical_size.width, physical_size.height, scale);

        // Update our logical size tracking
        let logical_size = info.logical_size();
        self.state
            .size
            .store((logical_size.width as u32, logical_size.height as u32));

        // Notify Slint of the new size (logical)
        self.adapter
            .window
            .dispatch_event(slint::platform::WindowEvent::Resized {
                size: slint::LogicalSize::new(
                    logical_size.width as f32,
                    logical_size.height as f32,
                ),
            });

        // Also set the scale factor on the Slint window
        self.adapter
            .window
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            });
    }

    /// Queue a resize to be applied next frame. Use this from Slint callbacks where
    /// you don't have access to `&mut Window`.
    pub fn queue_resize(&self, width: u32, height: u32) {
        self.pending_resizes.borrow_mut().push((width, height));
    }

    /// Returns the resize queue so you can clone the `Rc` and push to it from callbacks.
    pub fn pending_resizes(&self) -> &Rc<RefCell<Vec<(u32, u32)>>> {
        &self.pending_resizes
    }

    pub fn process_pending_resizes(&self, window: &mut baseview::Window) -> Option<(u32, u32)> {
        let mut queue = self.pending_resizes.borrow_mut();
        if let Some((width, height)) = queue.pop() {
            // Only process the most recent resize request to avoid lag
            queue.clear();
            drop(queue); // Release the borrow before calling resize

            self.resize(window, width, height);
            Some((width, height))
        } else {
            None
        }
    }

    pub fn component(&self) -> &T {
        &self.component
    }

    pub fn window(&self) -> &slint::Window {
        &self.adapter.window
    }

    // pub fn context(&self) -> &Arc<dyn GuiContext> {
    //     &self.context
    // }

    pub fn set_parameter_normalized(
        &self,
        param: &impl nice_plug_core::params::Param,
        normalized: f32,
    ) {
        // let setter = ParamSetter::new(&*self.context);
        // setter.set_parameter_normalized(param, normalized);
    }

    pub fn begin_set_parameter(&self, param: &impl nice_plug_core::params::Param) {
        // let setter = ParamSetter::new(&*self.context);
        // setter.begin_set_parameter(param);
    }

    pub fn end_set_parameter(&self, param: &impl nice_plug_core::params::Param) {
        // let setter = ParamSetter::new(&*self.context);
        // setter.end_set_parameter(param);
    }

    pub fn set_prevent_key_event_propagation(&self, is_enabled: bool) {
        *self.prevent_key_event_propagation.borrow_mut() = is_enabled;
    }
}

impl<T: slint::ComponentHandle> baseview::WindowHandler for WindowHandler<T> {
    fn on_frame(&mut self, window: &mut baseview::Window) {
        // Make the GL context current for this frame.
        unsafe { window.gl_context().unwrap().make_current() };

        // On first frame: initialize the renderer and show the component.
        // We defer this until on_frame (rather than doing it in spawn's closure)
        // so the GL context is guaranteed to be current when FemtoVG queries GL_VERSION.
        if !*self.window_shown.borrow() {
            self.adapter.set_gl_context(window);
            let _ = self.adapter.renderer.get_or_init(|| {
                self.adapter
                    .gl_interface
                    .get()
                    .map(|iface| {
                        FemtoVGRenderer::new(iface.clone())
                            .expect("Failed to create FemtoVG renderer")
                    })
                    .expect("gl_interface must be set before renderer init")
            });
            self.component.show().expect("Failed to show component");
            *self.window_shown.borrow_mut() = true;

            // This fires once, allowing users to register parameter update callbacks for UI -> plugin one time before the event loop starts
            (self.setup_handler)(self, window);
        }

        // Call custom event loop handler first
        // let setter = ParamSetter::new(&*self.context);
        // (self.event_loop_handler)(&self, setter, window);

        // Update Slint timers and animations
        slint::platform::update_timers_and_animations();

        // Process pending resizes
        self.process_pending_resizes(window);

        // Render the component - Slint handles the rendering internally
        // It will call our WindowAdapter's renderer() method when needed
        self.component.window().request_redraw();

        // Process Slint's internal rendering queue
        // This is where Slint actually renders using our FemtoVG renderer
        slint::platform::duration_until_next_timer_update();

        // CRITICAL: Actually trigger the render by accessing the renderer
        // Slint's FemtoVG renderer needs to be explicitly told to render
        if let Some(renderer) = self.adapter.renderer.get() {
            let _ = renderer.render();
        }

        // Swap buffers after rendering
        window.gl_context().unwrap().swap_buffers();
    }

    fn on_event(&mut self, _window: &mut baseview::Window, event: Event) -> EventStatus {
        match event {
            Event::Mouse(mouse_event) => {
                // Convert baseview mouse event to Slint event
                let slint_event = match mouse_event {
                    baseview::MouseEvent::CursorMoved { position, .. } => {
                        let pos = LogicalPosition::new(position.x as f32, position.y as f32);
                        *self.last_cursor_pos.borrow_mut() = pos;
                        WindowEvent::PointerMoved { position: pos }
                    }
                    baseview::MouseEvent::ButtonPressed { button, .. } => {
                        let slint_button = match button {
                            baseview::MouseButton::Left => {
                                slint::platform::PointerEventButton::Left
                            }
                            baseview::MouseButton::Right => {
                                slint::platform::PointerEventButton::Right
                            }
                            baseview::MouseButton::Middle => {
                                slint::platform::PointerEventButton::Middle
                            }
                            _ => return EventStatus::Ignored,
                        };
                        WindowEvent::PointerPressed {
                            button: slint_button,
                            position: *self.last_cursor_pos.borrow(),
                        }
                    }
                    baseview::MouseEvent::ButtonReleased { button, .. } => {
                        let slint_button = match button {
                            baseview::MouseButton::Left => {
                                slint::platform::PointerEventButton::Left
                            }
                            baseview::MouseButton::Right => {
                                slint::platform::PointerEventButton::Right
                            }
                            baseview::MouseButton::Middle => {
                                slint::platform::PointerEventButton::Middle
                            }
                            _ => return EventStatus::Ignored,
                        };
                        WindowEvent::PointerReleased {
                            button: slint_button,
                            position: *self.last_cursor_pos.borrow(),
                        }
                    }
                    baseview::MouseEvent::WheelScrolled { delta, .. } => {
                        let (delta_x, delta_y) = match delta {
                            baseview::ScrollDelta::Lines { x, y } => (x * 20.0, y * 20.0),
                            baseview::ScrollDelta::Pixels { x, y } => (x, y),
                        };
                        WindowEvent::PointerScrolled {
                            position: LogicalPosition::new(0.0, 0.0),
                            delta_x: delta_x as f32,
                            delta_y: delta_y as f32,
                        }
                    }
                    _ => return EventStatus::Ignored,
                };

                self.adapter.window.dispatch_event(slint_event);
                EventStatus::Captured
            }
            Event::Keyboard(key_event) => {
                let text: SharedString = if let keyboard_types::Key::Character(char) = key_event.key
                {
                    char.into()
                } else {
                    match key_event.code {
                        keyboard_types::Code::Enter => slint::platform::Key::Return.into(),
                        keyboard_types::Code::Tab => slint::platform::Key::Tab.into(),
                        keyboard_types::Code::Space => slint::platform::Key::Space.into(),
                        keyboard_types::Code::Backspace => slint::platform::Key::Backspace.into(),
                        keyboard_types::Code::Escape => slint::platform::Key::Escape.into(),
                        keyboard_types::Code::ArrowUp => slint::platform::Key::UpArrow.into(),
                        keyboard_types::Code::ArrowDown => slint::platform::Key::DownArrow.into(),
                        keyboard_types::Code::ArrowLeft => slint::platform::Key::LeftArrow.into(),
                        keyboard_types::Code::ArrowRight => slint::platform::Key::RightArrow.into(),
                        keyboard_types::Code::ShiftLeft => slint::platform::Key::Shift.into(),
                        keyboard_types::Code::ShiftRight => slint::platform::Key::ShiftR.into(),
                        keyboard_types::Code::ControlLeft => slint::platform::Key::Control.into(),
                        keyboard_types::Code::ControlRight => slint::platform::Key::ControlR.into(),
                        keyboard_types::Code::AltLeft => slint::platform::Key::Alt.into(),
                        keyboard_types::Code::AltRight => slint::platform::Key::AltGr.into(),
                        keyboard_types::Code::MetaLeft => slint::platform::Key::Meta.into(),
                        keyboard_types::Code::MetaRight => slint::platform::Key::MetaR.into(),
                        _ => "".into(),
                    }
                };

                if text.is_empty() {
                    return EventStatus::Ignored;
                }

                match key_event.state {
                    keyboard_types::KeyState::Down => {
                        if key_event.repeat {
                            self.adapter
                                .window
                                .dispatch_event(WindowEvent::KeyPressRepeated { text });
                        } else {
                            self.adapter
                                .window
                                .dispatch_event(WindowEvent::KeyPressed { text });
                        }
                    }
                    keyboard_types::KeyState::Up => {
                        self.adapter
                            .window
                            .dispatch_event(WindowEvent::KeyReleased { text });
                    }
                }

                if *self.prevent_key_event_propagation.borrow() {
                    EventStatus::Captured
                } else {
                    EventStatus::Ignored
                }
            }
            Event::Window(window_event) => {
                match window_event {
                    BaseviewWindowEvent::Resized(info) => {
                        // Handle scale factor and size changes from baseview
                        self.handle_window_info(&info);
                        EventStatus::Captured
                    }
                    BaseviewWindowEvent::Focused => EventStatus::Ignored,
                    BaseviewWindowEvent::Unfocused => EventStatus::Ignored,
                    BaseviewWindowEvent::WillClose => EventStatus::Ignored,
                }
            }
        }
    }
}
