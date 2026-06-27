use baseview::{gl::GlConfig, Size, Window, WindowHandle, WindowOpenOptions, WindowScalePolicy};
use crossbeam::atomic::AtomicCell;
use nice_plug_core::context::gui::GuiContext;
use nice_plug_core::context::gui::ParamSetter;
use nice_plug_core::editor::Editor;
use nice_plug_core::params::persist::PersistentField;
use serde::{Deserialize, Serialize};
use slint::LogicalPosition;
use std::{cell::RefCell, rc::Rc, sync::Arc};

use crate::window_adapter::BaseviewSlintAdapter;

pub mod open_gl_interface;
pub mod platform;
pub mod window_adapter;
pub mod window_handler;

type EventLoopHandler<T> =
    dyn Fn(&window_handler::WindowHandler<T>, ParamSetter, &mut Window) + Send + Sync;
type SetupHandler<T> = dyn Fn(&window_handler::WindowHandler<T>, &mut Window) + Send + Sync;

// Thread-local storage for the current adapter
// Holds the active adapter so that BaseviewSlintPlatform::create_window_adapter can
// return it.  Updated each time a window is opened.
thread_local! {
    pub static CURRENT_ADAPTER: RefCell<Option<Rc<BaseviewSlintAdapter>>> = RefCell::new(None);
}

/// Window size/state that gets persisted via nice-plug's `#[persist]` mechanism.
///
/// Put this in your params struct so the host can save and restore the window size:
///
/// ```rust,ignore
/// #[derive(Params)]
/// struct MyParams {
///     #[persist = "editor-state"]
///     editor_state: Arc<SlintEditorState>,
/// }
/// ```
#[derive(Debug, Serialize, Deserialize)]
pub struct SlintEditorState {
    #[serde(with = "nice_plug_core::params::persist::serialize_atomic_cell")]
    pub size: AtomicCell<(u32, u32)>,
}

fn default_width() -> u32 {
    400
}
fn default_height() -> u32 {
    300
}

impl<'a> PersistentField<'a, SlintEditorState> for Arc<SlintEditorState> {
    fn set(&self, new_value: SlintEditorState) {
        self.size.store(new_value.size.load());
    }

    fn map<F, R>(&self, f: F) -> R
    where
        F: Fn(&SlintEditorState) -> R,
    {
        f(self)
    }
}

impl Default for SlintEditorState {
    fn default() -> Self {
        Self {
            size: AtomicCell::new((default_width(), default_height())),
        }
    }
}

impl SlintEditorState {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            size: AtomicCell::new((width, height)),
        }
    }

    /// Returns a `(width, height)` pair for the current size of the GUI in logical pixels.
    pub fn size(&self) -> (u32, u32) {
        self.size.load()
    }
}

/// The nice-plug [`Editor`] implementation for Slint UIs.
///
/// Build one with [`SlintEditor::new`], optionally chaining
/// [`with_event_loop`][Self::with_event_loop] to sync parameters each frame.
///
/// ```rust,ignore
/// fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
///     Some(Box::new(
///         SlintEditor::new(self.params.editor_state.clone(), || gui::AppWindow::new())
///             .with_event_loop({
///                 let params = self.params.clone();
///                 move |handler, _setter, _window| {
///                     handler.component().set_gain(params.gain.value());
///                 }
///             }),
///     ))
/// }
/// ```
pub struct SlintEditor<T: slint::ComponentHandle> {
    component_factory: Arc<dyn Fn() -> Result<T, slint::PlatformError> + Send + Sync>,
    state: Arc<SlintEditorState>,
    event_loop_handler: Arc<EventLoopHandler<T>>,
    setup_handler: Arc<SetupHandler<T>>,
}

impl<T: slint::ComponentHandle + 'static> SlintEditor<T> {
    /// Create an editor from persisted state and a component factory closure.
    pub fn new<F>(state: Arc<SlintEditorState>, factory: F) -> Self
    where
        F: Fn() -> Result<T, slint::PlatformError> + 'static + Send + Sync,
    {
        Self {
            component_factory: Arc::new(factory),
            state,
            event_loop_handler: Arc::new(|_, _, _| {}),
            setup_handler: Arc::new(|_, _| {}),
        }
    }

    pub fn with_setup<F>(mut self, handler: F) -> Self
    where
        F: Fn(&window_handler::WindowHandler<T>, &mut Window) + 'static + Send + Sync,
    {
        self.setup_handler = Arc::new(handler);
        self
    }

    /// Set the handler called every frame. Use it to push parameter values to the UI
    /// and register Slint callbacks for UI → plugin communication.
    pub fn with_event_loop<F>(mut self, handler: F) -> Self
    where
        F: Fn(&window_handler::WindowHandler<T>, ParamSetter, &mut Window) + 'static + Send + Sync,
    {
        self.event_loop_handler = Arc::new(handler);
        self
    }
}

struct Instance {
    window_handle: WindowHandle,
}

impl Drop for Instance {
    fn drop(&mut self) {
        self.window_handle.close();
    }
}

// SAFETY: `Instance` only contains a `WindowHandle`, which is not `Send` because it holds
// a raw pointer to the platform window.  However, we only ever close the window from the
// audio thread (via `Drop`), and baseview guarantees that `WindowHandle::close` is safe to
// call from any thread.
unsafe impl Send for Instance {}

impl<T: slint::ComponentHandle + 'static> Editor for SlintEditor<T> {
    fn spawn(
        &self,
        parent: nice_plug_core::editor::ParentWindowHandle,
        context: Arc<dyn GuiContext>,
    ) -> Box<dyn std::any::Any + Send> {
        let (width, height) = self.state.size();
        let options = WindowOpenOptions {
            scale: WindowScalePolicy::SystemScaleFactor,
            size: Size {
                width: width as f64,
                height: height as f64,
            },
            title: "Plug-in".to_owned(),
            // Request OpenGL context for FemtoVG rendering
            gl_config: Some(GlConfig {
                version: (3, 2),
                red_bits: 8,
                blue_bits: 8,
                green_bits: 8,
                alpha_bits: 8,
                depth_bits: 24,
                stencil_bits: 8,
                samples: None,
                srgb: true,
                double_buffer: true,
                vsync: false,
                ..Default::default()
            }),
        };

        let state = self.state.clone();
        let event_loop_handler = self.event_loop_handler.clone();
        let setup_handler = self.setup_handler.clone();
        let component_factory = self.component_factory.clone();

        let window_handle =
            baseview::Window::open_parented(&parent, options, move |baseview_window| {
                // let gl_context = Arc::new(baseview_window.gl_context().unwrap());
                // Make the GL context current so that any renderer creation during component
                // initialization (Slint may call renderer() eagerly) has a valid context.
                unsafe { baseview_window.gl_context().unwrap().make_current() };

                // Create the Slint window adapter.
                // Start with scale 1.0; the actual system scale arrives via the first
                // WindowEvent::Resized from baseview.
                let initial_scale = 1.0f32;
                let adapter = BaseviewSlintAdapter::new(width, height, initial_scale);

                // Wire up the GL proc-address loader now that the context is live.
                adapter.set_gl_context(baseview_window);

                // Register this adapter so BaseviewSlintPlatform::create_window_adapter returns it.
                CURRENT_ADAPTER.with(|current| {
                    *current.borrow_mut() = Some(adapter.clone());
                });

                // Install our platform on first open; ignored (returns Err) on subsequent opens
                // since Slint only allows setting the platform once per process.
                let _ = slint::platform::set_platform(Box::new(platform::BaseviewSlintPlatform));

                let component = component_factory()
                    .unwrap_or_else(|e| panic!("Failed to create Slint component: {}", e));

                // Defer show() until on_frame so the GL context is current when FemtoVG
                // queries GL_VERSION during its first render.

                window_handler::WindowHandler {
                    // context: gl_context,
                    // event_loop_handler,
                    setup_handler,
                    scale_factor: RefCell::new(initial_scale),
                    state,
                    pending_resizes: Rc::new(RefCell::new(Vec::new())),
                    last_cursor_pos: RefCell::new(LogicalPosition::new(0.0, 0.0)),
                    window_shown: RefCell::new(false),
                    component,
                    adapter,
                    prevent_key_event_propagation: RefCell::new(false),
                }
            });

        Box::new(Instance { window_handle })
    }

    fn size(&self) -> (u32, u32) {
        self.state.size()
    }

    fn set_scale_factor(&self, _factor: f32) -> bool {
        // TODO: Implement scale factor handling for Slint
        false
    }

    fn param_values_changed(&self) {}

    fn param_value_changed(&self, _id: &str, _normalized_value: f32) {}

    fn param_modulation_changed(&self, _id: &str, _modulation_offset: f32) {}
}
