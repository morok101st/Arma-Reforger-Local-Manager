//! Kleine Hilfen für Windows-Aufrufe.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::core::PCWSTR;

/// Text als nullterminiertes UTF-16 für Windows.
pub fn wide(text: impl AsRef<OsStr>) -> Vec<u16> {
    text.as_ref().encode_wide().chain(std::iter::once(0)).collect()
}

pub fn pcwstr(wide: &[u16]) -> PCWSTR {
    PCWSTR(wide.as_ptr())
}

/// Schließt das Handle beim Verlassen.
#[derive(Debug)]
pub struct OwnedHandle(pub HANDLE);

// Ein Windows-Handle darf von jedem Thread benutzt und geschlossen werden.
unsafe impl Send for OwnedHandle {}
unsafe impl Sync for OwnedHandle {}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            // SAFETY: Das Handle gehört nur dieser Hülle und wird genau einmal geschlossen.
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}
