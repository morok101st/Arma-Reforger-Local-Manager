//! Es kann immer nur ein Client laufen. Die Sperre gilt profilübergreifend und auch zwischen der WPF-App
//! und dieser Ausgabe, deshalb derselbe Name.

use windows::Win32::Foundation::WAIT_OBJECT_0;
use windows::Win32::System::Threading::{CreateSemaphoreW, ReleaseSemaphore, WaitForSingleObject};

use crate::win::{OwnedHandle, pcwstr, wide};

pub const SYNC_LOCK_NAME: &str = r"Local\ReforgerModSync.Sync";

/// Gehaltene Sperre. Wird beim Verlassen freigegeben.
#[derive(Debug)]
pub struct SyncLock {
    handle: OwnedHandle,
}

impl SyncLock {
    /// `None`: Es läuft bereits ein Sync, in dieser oder der anderen App.
    pub fn try_acquire() -> Option<Self> {
        Self::try_acquire_named(SYNC_LOCK_NAME)
    }

    /// Mit eigenem Namen, damit Tests die echte Sperre nicht berühren.
    pub fn try_acquire_named(name: &str) -> Option<Self> {
        let name = wide(name);
        // Semaphore statt Mutex: Ein Mutex wäre an den Thread gebunden, der Lauf wechselt aber Threads.
        // SAFETY: Name ist nullterminiert, das Handle gehört danach der Hülle.
        let handle = OwnedHandle(unsafe { CreateSemaphoreW(None, 1, 1, pcwstr(&name)) }.ok()?);
        // SAFETY: Gültiges Handle, Wartezeit 0.
        (unsafe { WaitForSingleObject(handle.0, 0) } == WAIT_OBJECT_0).then_some(Self { handle })
    }
}

impl Drop for SyncLock {
    fn drop(&mut self) {
        // SAFETY: Diese Hülle hat die Semaphore genau einmal belegt.
        unsafe {
            let _ = ReleaseSemaphore(self.handle.0, 1, None);
        }
    }
}
