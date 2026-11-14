//! Ein laufender Client. Ein Job Object beendet ihn mit, falls das Tool abstürzt.

use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{HANDLE, HWND, LPARAM, WPARAM};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, SW_HIDE, ShowWindow, WM_CLOSE,
};
use windows::core::{BOOL, PCWSTR};

use super::{ClientProcess, StopFuture};
use crate::win::OwnedHandle;

const HIDE_INTERVAL_AT_START: Duration = Duration::from_millis(50);
const HIDE_INTERVAL: Duration = Duration::from_millis(500);
const START_PHASE: Duration = Duration::from_secs(30);
const CLOSE_GRACE: Duration = Duration::from_secs(5);

pub struct ClientSession {
    child: Mutex<Child>,
    process_id: u32,
    job: OwnedHandle,
    stop_hiding: Arc<AtomicBool>,
}

impl ClientSession {
    /// Startet den Client. `hide_window` blendet das Spielfenster aus, es bleibt beim Start nur einen Moment sichtbar.
    pub fn start(exe: &Path, arguments: &[String], hide_window: bool) -> io::Result<Self> {
        let job = create_kill_on_close_job()?;
        let mut command = Command::new(exe);
        // Die Konsolenausgabe des Clients (u. a. die Steam-ID) gehört nicht in Ausgabe oder Logs der App.
        command.args(arguments).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
        if let Some(folder) = exe.parent() {
            command.current_dir(folder);
        }
        let mut child = command.spawn()?;

        let process = HANDLE(child.as_raw_handle());
        // SAFETY: Beide Handles sind gültig, solange `child` und `job` leben.
        if let Err(error) = unsafe { AssignProcessToJobObject(job.0, process) } {
            let _ = child.kill();
            return Err(io::Error::other(error.message()));
        }

        let session = Self { process_id: child.id(), child: Mutex::new(child), job, stop_hiding: Arc::new(AtomicBool::new(false)) };
        if hide_window {
            keep_hidden(session.process_id, Arc::clone(&session.stop_hiding));
        }
        Ok(session)
    }

    async fn wait_exited(&self, max_wait: Duration) -> bool {
        let started = Instant::now();
        while !self.has_exited() {
            if started.elapsed() >= max_wait {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        true
    }

    /// Erst sauber schließen, sonst alle Prozesse des Jobs beenden.
    async fn stop_async(&self) {
        self.stop_hiding.store(true, Ordering::SeqCst);
        if self.has_exited() {
            return;
        }

        for window in windows_of(self.process_id) {
            // SAFETY: Ein Fenster, das inzwischen weg ist, lässt den Aufruf nur scheitern.
            unsafe {
                let _ = PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0));
            }
        }

        if !self.wait_exited(CLOSE_GRACE).await {
            // SAFETY: Das Job-Handle ist gültig, solange die Sitzung lebt.
            unsafe {
                let _ = TerminateJobObject(self.job.0, 1);
            }
            self.wait_exited(CLOSE_GRACE).await;
        }
    }
}

impl ClientProcess for ClientSession {
    fn has_exited(&self) -> bool {
        self.child.lock().map(|mut child| !matches!(child.try_wait(), Ok(None))).unwrap_or(true)
    }

    fn stop(&self) -> StopFuture<'_> {
        Box::pin(self.stop_async())
    }
}

impl Drop for ClientSession {
    fn drop(&mut self) {
        self.stop_hiding.store(true, Ordering::SeqCst);
    }
}

fn create_kill_on_close_job() -> io::Result<OwnedHandle> {
    // SAFETY: Ohne Name und Sicherheitsattribute, das Handle gehört danach der Hülle.
    let job = OwnedHandle(unsafe { CreateJobObjectW(None, PCWSTR::null()) }.map_err(|error| io::Error::other(error.message()))?);
    let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: `info` hat genau die angegebene Größe.
    unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const info).cast(),
            u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).expect("klein"),
        )
    }
    .map_err(|error| io::Error::other(error.message()))?;
    Ok(job)
}

/// Läuft dauerhaft, weil das Spiel beim Laden weitere Fenster öffnen oder seines wieder zeigen kann.
fn keep_hidden(process_id: u32, stop: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let started = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            for window in windows_of(process_id) {
                // SAFETY: Nur Abfragen und Ausblenden, ein ungültiges Fenster lässt die Aufrufe scheitern.
                unsafe {
                    if IsWindowVisible(window).as_bool() {
                        let _ = ShowWindow(window, SW_HIDE);
                    }
                }
            }
            std::thread::sleep(if started.elapsed() < START_PHASE { HIDE_INTERVAL_AT_START } else { HIDE_INTERVAL });
        }
    });
}

fn windows_of(process_id: u32) -> Vec<HWND> {
    struct Search {
        process_id: u32,
        windows: Vec<HWND>,
    }

    unsafe extern "system" fn collect(window: HWND, parameter: LPARAM) -> BOOL {
        // SAFETY: `parameter` zeigt auf die `Search` unten, die während EnumWindows lebt.
        let search = unsafe { &mut *(parameter.0 as *mut Search) };
        let mut owner = 0;
        // SAFETY: `owner` ist ein gültiger Zeiger.
        unsafe { GetWindowThreadProcessId(window, Some(&raw mut owner)) };
        if owner == search.process_id {
            search.windows.push(window);
        }
        BOOL::from(true)
    }

    let mut search = Search { process_id, windows: Vec::new() };
    // SAFETY: Der Rückruf nutzt den Zeiger nur während dieses Aufrufs.
    unsafe {
        let _ = EnumWindows(Some(collect), LPARAM((&raw mut search) as isize));
    }
    search.windows
}
