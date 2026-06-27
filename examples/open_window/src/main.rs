use std::num::NonZeroU32;

#[cfg(target_os = "macos")]
use baseview::copy_to_clipboard;
use baseview::{
    Event, EventStatus, MouseEvent, PhyPoint, PhySize, Window, WindowEvent, WindowHandler,
    WindowInfo, WindowOpenOptions,
};

struct OpenWindowExample {
    _ctx: softbuffer::Context,
    surface: softbuffer::Surface,
    current_size: WindowInfo,
    mouse_pos: PhyPoint,
    is_cursor_inside: bool,
    damaged: bool,
}

impl WindowHandler for OpenWindowExample {
    fn on_frame(&mut self, _window: &mut Window) {
        let mut pixels = self.surface.buffer_mut().unwrap();
        if !self.damaged {
            return;
        }
        pixels.fill(0xFFAAAAAA);

        pixels.present().unwrap();
        self.damaged = false;
    }

    fn on_event(&mut self, _window: &mut Window, event: Event) -> EventStatus {
        match &event {
            #[cfg(target_os = "macos")]
            Event::Mouse(MouseEvent::ButtonPressed { .. }) => copy_to_clipboard("This is a test!"),
            Event::Mouse(MouseEvent::CursorMoved { position, .. }) => {
                let phy_pos = position.to_physical(&self.current_size);
                self.mouse_pos = phy_pos;
                self.damaged = true;
            }
            Event::Mouse(MouseEvent::CursorEntered) => {
                self.is_cursor_inside = true;
                self.damaged = true;
            }
            Event::Mouse(MouseEvent::CursorLeft) => {
                self.is_cursor_inside = false;
                self.damaged = true;
            }
            Event::Window(WindowEvent::Resized(info)) => {
                println!("Resized: {:?}", info);
                self.current_size = *info;

                let new_size = info.physical_size();

                if let (Some(width), Some(height)) = (
                    NonZeroU32::new(new_size.width),
                    NonZeroU32::new(new_size.height),
                ) {
                    self.surface.resize(width, height).unwrap();
                    self.damaged = true;
                }
            }
            _ => {}
        }

        log_event(&event);

        EventStatus::Captured
    }
}

fn main() {
    let window_open_options = WindowOpenOptions::new().with_size(512.0, 512.0);

    Window::open_blocking(window_open_options, |window| {
        let ctx = unsafe { softbuffer::Context::new(window) }.unwrap();
        let mut surface = unsafe { softbuffer::Surface::new(&ctx, window) }.unwrap();
        surface
            .resize(NonZeroU32::new(512).unwrap(), NonZeroU32::new(512).unwrap())
            .unwrap();

        let current_size = WindowInfo::from_physical_size(PhySize::new(512, 512), 1.0);
        println!("current_size in open blocking: {:?}", current_size);
        OpenWindowExample {
            _ctx: ctx,
            surface,
            current_size,
            mouse_pos: PhyPoint::new(0, 0),
            is_cursor_inside: false,
            damaged: true,
        }
    });
}

fn log_event(event: &Event) {
    match event {
        Event::Mouse(e) => println!("Mouse event: {:?}", e),
        Event::Keyboard(e) => println!("Keyboard event: {:?}", e),
        Event::Window(e) => println!("Window event: {:?}", e),
    }
}
