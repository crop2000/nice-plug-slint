use baseview::{
    gl::GlConfig, Event, EventStatus, PhySize, Size, Window, WindowEvent, WindowHandle,
    WindowHandler, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};
use nice_plug_slint::{
    platform, window_adapter::BaseviewSlintAdapter, window_handler, SlintEditorState,
    CURRENT_ADAPTER,
};
use slint::LogicalPosition;
use std::{cell::RefCell, num::NonZeroU32, rc::Rc, sync::Arc};

mod gui;

struct ParentWindowHandler {
    _ctx: softbuffer::Context,
    surface: softbuffer::Surface,
    current_size: WindowInfo,
    damaged: bool,

    _child_window: Option<WindowHandle>,
}

impl ParentWindowHandler {
    pub fn new(parent: &mut Window) -> Self {
        let ctx = unsafe { softbuffer::Context::new(parent) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, parent) }.unwrap();
        surface
            .resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap())
            .unwrap();

        let options = WindowOpenOptions::new()
            .with_size(256.0, 256.0)
            .with_title("baseview child")
            .with_gl_config(GlConfig {
                alpha_bits: 8,
                ..GlConfig::default()
            });
        // .with_gl_config(Some(GlConfig {
        //     version: (3, 2),
        //     red_bits: 8,
        //     blue_bits: 8,
        //     green_bits: 8,
        //     alpha_bits: 8,
        //     depth_bits: 24,
        //     stencil_bits: 8,
        //     samples: None,
        //     srgb: true,
        //     double_buffer: true,
        //     vsync: false,
        //     ..Default::default()
        // }));

        // let child_window = Window::open_parented(parent, options, ChildWindowHandler::new);
        //
        // let options = WindowOpenOptions {
        //     scale: WindowScalePolicy::SystemScaleFactor,
        //     size: Size {
        //         width: 256.0,
        //         height: 256.0,
        //     },
        //     title: "Plug-in".to_owned(),
        //     // Request OpenGL context for FemtoVG rendering
        //     gl_config: Some(GlConfig {
        //         version: (3, 2),
        //         red_bits: 8,
        //         blue_bits: 8,
        //         green_bits: 8,
        //         alpha_bits: 8,
        //         depth_bits: 24,
        //         stencil_bits: 8,
        //         samples: None,
        //         srgb: true,
        //         double_buffer: true,
        //         vsync: false,
        //         ..Default::default()
        //     }),
        // };

        let child_window =
            baseview::Window::open_parented(parent, options, move |baseview_window| {
                // Make the GL context current so that any renderer creation during component
                // initialization (Slint may call renderer() eagerly) has a valid context.
                unsafe { baseview_window.gl_context().unwrap().make_current() };

                // Create the Slint window adapter.
                // Start with scale 1.0; the actual system scale arrives via the first
                // WindowEvent::Resized from baseview.
                let initial_scale = 1.0f32;
                let adapter = BaseviewSlintAdapter::new(256, 256, initial_scale);

                // Wire up the GL proc-address loader now that the context is live.
                adapter.set_gl_context(baseview_window);

                // Register this adapter so BaseviewSlintPlatform::create_window_adapter returns it.
                CURRENT_ADAPTER.with(|current| {
                    *current.borrow_mut() = Some(adapter.clone());
                });

                // Install our platform on first open; ignored (returns Err) on subsequent opens
                // since Slint only allows setting the platform once per process.
                let _ = slint::platform::set_platform(Box::new(platform::BaseviewSlintPlatform));

                let component = gui::AppWindow::new()
                    .unwrap_or_else(|e| panic!("Failed to create Slint component: {}", e));

                // Defer show() until on_frame so the GL context is current when FemtoVG
                // queries GL_VERSION during its first render.

                let state = Arc::new(SlintEditorState::new(200, 220));
                window_handler::WindowHandler {
                    // context,
                    // event_loop_handler,
                    setup_handler: Arc::new(|_, _| {}),
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
        let current_size = WindowInfo::from_physical_size(PhySize::new(512, 512), 1.0);
        println!("current_size in open blocking: {:?}", current_size);

        // TODO: no way to query physical size initially?
        Self {
            _ctx: ctx,
            surface,
            current_size,
            damaged: true,
            _child_window: Some(child_window),
        }
    }
}

impl WindowHandler for ParentWindowHandler {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut buf = self.surface.buffer_mut().unwrap();
        if self.damaged {
            buf.fill(0xFFAAAAAA);
            self.damaged = false;
        }
        buf.present().unwrap();
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Parent Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size = info;

                if let (Some(width), Some(height)) = (
                    NonZeroU32::new(new_size.width),
                    NonZeroU32::new(new_size.height),
                ) {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
                }
            }
            Event::Mouse(e) => println!("Parent Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Parent Keyboard event: {:?}", e),
            Event::Window(e) => println!("Parent Window event: {:?}", e),
        }

        EventStatus::Captured
    }
}

struct ChildWindowHandler {
    _ctx: softbuffer::Context,
    surface: softbuffer::Surface,
    current_size: PhySize,
    damaged: bool,
}

impl ChildWindowHandler {
    pub fn new(window: &mut Window) -> Self {
        let ctx = unsafe { softbuffer::Context::new(window) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, window) }.unwrap();
        surface
            .resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap())
            .unwrap();

        // TODO: no way to query physical size initially?
        Self {
            _ctx: ctx,
            surface,
            current_size: PhySize::new(256, 256),
            damaged: true,
        }
    }
}

impl WindowHandler for ChildWindowHandler {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut buf = self.surface.buffer_mut().unwrap();
        if self.damaged {
            buf.fill(0xFFAA0000);
            self.damaged = false;
        }
        buf.present().unwrap();
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match event {
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Child Resized: {:?}", info);
                let new_size = info.physical_size();
                self.current_size = new_size;

                if let (Some(width), Some(height)) = (
                    NonZeroU32::new(new_size.width),
                    NonZeroU32::new(new_size.height),
                ) {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
                }
            }
            Event::Mouse(e) => println!("Child Mouse event: {:?}", e),
            Event::Keyboard(e) => println!("Child Keyboard event: {:?}", e),
            Event::Window(e) => println!("Child Window event: {:?}", e),
        }

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowOpenOptions::new().with_size(512.0, 512.0);

    Window::open_blocking(window_open_options, ParentWindowHandler::new);
}
