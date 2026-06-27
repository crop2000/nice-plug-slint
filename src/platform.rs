use slint::platform::WindowAdapter;

use std::rc::Rc;

use crate::CURRENT_ADAPTER;

/// Platform implementation for Slint
pub struct BaseviewSlintPlatform;

impl slint::platform::Platform for BaseviewSlintPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        CURRENT_ADAPTER.with(|adapter| {
            adapter
                .borrow()
                .clone()
                .map(|a| a as Rc<dyn WindowAdapter>)
                .ok_or_else(|| slint::PlatformError::Other("No adapter set".into()))
        })
    }
}
