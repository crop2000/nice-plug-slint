use super::open_gl_interface::BaseviewOpenGLInterface;
use once_cell::unsync::OnceCell;
use slint::platform::femtovg_renderer::FemtoVGRenderer;
use slint::platform::WindowAdapter;
use slint::PhysicalSize;
use std::cell::RefCell;
use std::rc::Rc;

/// Custom WindowAdapter that bridges baseview and Slint
pub struct BaseviewSlintAdapter {
    pub window: slint::Window,
    pub renderer: OnceCell<FemtoVGRenderer>,
    /// Physical size in actual pixels (for the OpenGL framebuffer)
    physical_size: RefCell<PhysicalSize>,
    /// Scale factor (e.g., 2.0 on Retina displays)
    scale_factor: RefCell<f32>,
    /// Stored proc-address loader, set once the GL context is available
    pub gl_interface: OnceCell<BaseviewOpenGLInterface>,
}

impl BaseviewSlintAdapter {
    pub fn new(physical_width: u32, physical_height: u32, scale_factor: f32) -> Rc<Self> {
        Rc::new_cyclic(|weak_self| {
            let window = slint::Window::new(weak_self.clone() as _);
            Self {
                window,
                renderer: OnceCell::new(),
                physical_size: RefCell::new(PhysicalSize::new(physical_width, physical_height)),
                scale_factor: RefCell::new(scale_factor),
                gl_interface: OnceCell::new(),
            }
        })
    }

    /// Call once after the GL context is live to wire up the proc-address loader.
    pub fn set_gl_context(&self, window: &baseview::Window) {
        let _ = self.gl_interface.set(BaseviewOpenGLInterface::new(window));
    }

    /// Update the size and scale factor (called when window is resized or scale changes)
    pub fn update_size(&self, physical_width: u32, physical_height: u32, scale_factor: f32) {
        *self.physical_size.borrow_mut() = PhysicalSize::new(physical_width, physical_height);
        *self.scale_factor.borrow_mut() = scale_factor;
    }
}

impl WindowAdapter for BaseviewSlintAdapter {
    fn window(&self) -> &slint::Window {
        &self.window
    }

    fn size(&self) -> PhysicalSize {
        *self.physical_size.borrow()
    }

    fn renderer(&self) -> &dyn slint::platform::Renderer {
        self.renderer.get_or_init(|| {
            let interface = self
                .gl_interface
                .get()
                .expect("GL context must be set via set_gl_context() before renderer() is called");
            FemtoVGRenderer::new(interface.clone()).expect("Failed to create FemtoVG renderer")
        })
    }

    fn request_redraw(&self) {
        // baseview handles redraws in on_frame
    }
}
