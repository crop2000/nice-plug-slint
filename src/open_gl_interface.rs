use slint::platform::femtovg_renderer::OpenGLInterface;
use std::sync::Arc;

/// OpenGL interface implementation for baseview.
///
/// Delegates all GL symbol resolution to baseview's `GlContext::get_proc_address`,
/// which handles the platform-specific details (WGL on Windows, dlsym on Unix).
///
/// The inner function is stored in an `Arc` so this type can be cheaply cloned
/// when the `FemtoVGRenderer` is created from inside `WindowAdapter::renderer()`.
#[derive(Clone)]
pub struct BaseviewOpenGLInterface {
    pub(crate) get_proc_address: Arc<dyn Fn(&str) -> *const core::ffi::c_void + Send + Sync>,
}

impl BaseviewOpenGLInterface {
    pub(crate) fn new(window: &baseview::Window) -> Self {
        // Store the GlContext address as a plain usize so that the closure is Send + Sync.
        //
        // SAFETY: The `GlContext` is owned by the `Window` and lives as long as the window is
        // open.  The `FemtoVGRenderer` (and therefore this interface) is dropped before the
        // window closes, so the pointer is valid for the entire lifetime of the renderer.
        // We only dereference it on the GUI thread (inside FemtoVG's GL loader callback).
        let ctx_addr = window
            .gl_context()
            .expect("window must have an OpenGL context")
            as *const baseview::gl::GlContext as usize;

        Self {
            get_proc_address: Arc::new(move |name: &str| {
                let ctx = ctx_addr as *const baseview::gl::GlContext;
                // SAFETY: see constructor comment above.
                unsafe { &*ctx }.get_proc_address(name)
            }),
        }
    }
}

unsafe impl OpenGLInterface for BaseviewOpenGLInterface {
    fn ensure_current(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // baseview makes the context current before calling on_frame
        Ok(())
    }

    fn swap_buffers(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // baseview handles buffer swapping (we call swap_buffers manually at end of frame)
        Ok(())
    }

    fn resize(
        &self,
        _width: core::num::NonZeroU32,
        _height: core::num::NonZeroU32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Resize is handled via WindowAdapter::size()
        Ok(())
    }

    fn get_proc_address(&self, name: &core::ffi::CStr) -> *const core::ffi::c_void {
        let name = name.to_str().unwrap_or("");
        (self.get_proc_address)(name)
    }
}
