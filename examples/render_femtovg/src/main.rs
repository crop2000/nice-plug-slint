use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use baseview::{
    gl::GlConfig, Event, EventStatus, MouseEvent, PhyPoint, PhySize, Size, Window, WindowEvent,
    WindowHandle as BaseviewWindowHandle, WindowInfo, WindowOpenOptions, WindowScalePolicy,
};
use femtovg::renderer::OpenGl;
use femtovg::{Canvas, Color};
use nice_plug_slint::window_handler::WindowHandler;
use nice_plug_slint::{
    platform, window_adapter::BaseviewSlintAdapter, window_handler, SlintEditorState,
    CURRENT_ADAPTER,
};
use slint::LogicalPosition;

use crate::gui::AppWindow;

mod gui;

struct FemtovgExample {
    canvas: Canvas<OpenGl>,
    current_size: WindowInfo,
    current_mouse_position: PhyPoint,
    damaged: bool,
}

// impl FemtovgExample {
fn new(window: &mut Window) -> WindowHandler<AppWindow> {
    let context = window.gl_context().unwrap();
    {
        // Make the GL context current so that any renderer creation during component
        // initialization (Slint may call renderer() eagerly) has a valid context.
        unsafe { context.make_current() };

        // Create the Slint window adapter.
        // Start with scale 1.0; the actual system scale arrives via the first
        // WindowEvent::Resized from baseview.
        let initial_scale = 1.0f32;
        let adapter = BaseviewSlintAdapter::new(256, 256, initial_scale);

        // Wire up the GL proc-address loader now that the context is live.
        adapter.set_gl_context(window);

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
    }

    // unsafe { context.make_current() };

    // let renderer =
    //     unsafe { OpenGl::new_from_function(|s| context.get_proc_address(s)) }.unwrap();

    // let mut canvas = Canvas::new(renderer).unwrap();
    // // TODO: get actual window width
    // canvas.set_size(512, 512, 1.0);

    // unsafe { context.make_not_current() };
    // Self {
    //     canvas,
    //     current_size: WindowInfo::from_logical_size(
    //         Size {
    //             width: 512.0,
    //             height: 512.0,
    //         },
    //         1.0,
    //     ),
    //     current_mouse_position: PhyPoint { x: 256, y: 256 },
    //     damaged: true,
    // }
}
// }

// impl WindowHandler for FemtovgExample {
//     fn on_frame(&mut self, window: &mut Window) {
//         if !self.damaged {
//             return;
//         }

//         let context = window.gl_context().unwrap();
//         unsafe { context.make_current() };

//         let screen_height = self.canvas.height();
//         let screen_width = self.canvas.width();

//         // Clear
//         self.canvas.clear_rect(
//             0,
//             0,
//             screen_width,
//             screen_height,
//             Color::rgb(0xAA, 0xAA, 0xAA),
//         );

//         // Make big blue rectangle
//         self.canvas.clear_rect(
//             (screen_width as f32 * 0.1).floor() as u32,
//             (screen_height as f32 * 0.1).floor() as u32,
//             (screen_width as f32 * 0.8).floor() as u32,
//             (screen_height as f32 * 0.8).floor() as u32,
//             Color::rgbf(0., 0.3, 0.9),
//         );

//         // Make smol orange rectangle
//         self.canvas.clear_rect(
//             (self.current_mouse_position.x - 15).clamp(0, screen_width as i32 - 30) as u32,
//             (self.current_mouse_position.y - 15).clamp(0, screen_height as i32 - 30) as u32,
//             30,
//             30,
//             Color::rgbf(0.9, 0.3, 0.),
//         );

//         // Tell renderer to execute all drawing commands
//         self.canvas.flush();
//         context.swap_buffers();
//         unsafe { context.make_not_current() };
//         self.damaged = false;
//     }

//     fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
//         match event {
//             Event::Window(WindowEvent::Resized(size)) => {
//                 let phy_size = size.physical_size();
//                 self.current_size = size;
//                 self.canvas
//                     .set_size(phy_size.width, phy_size.height, size.scale() as f32);
//                 self.damaged = true;
//             }
//             Event::Mouse(
//                 MouseEvent::CursorMoved { position, .. }
//                 | MouseEvent::DragEntered { position, .. }
//                 | MouseEvent::DragMoved { position, .. }
//                 | MouseEvent::DragDropped { position, .. },
//             ) => {
//                 self.current_mouse_position = position.to_physical(&self.current_size);
//                 self.damaged = true;
//             }
//             _ => {}
//         };
//         log_event(&event);
//         EventStatus::Captured
//     }
// }

fn main() {
    let window_open_options = WindowOpenOptions::new()
        .with_title("Femtovg on Baseview")
        .with_size(512.0, 512.0)
        .with_gl_config(GlConfig {
            alpha_bits: 8,
            ..GlConfig::default()
        });

    Window::open_blocking(window_open_options, new);
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
    }
}
