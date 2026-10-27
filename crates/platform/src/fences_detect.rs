//! Is Stardock Fences active right now (MistFences)? Read only: lists processes and the modules
//! loaded into Explorer, changes nothing.
//!
//! Fences 6 runs inside Explorer: `Fences.exe /startup` (a Run entry) loads its desktop
//! integration (`DesktopDock64.dll`, hooked in by `Detour64.dll`) into explorer.exe and exits.
//! Its context-menu extension (`FencesMenu64.dll`) also loads into Explorer, and into any
//! program that shows Explorer's context menus, while Fences is installed; it does not draw the
//! desktop, so it does not count as active.

use crate::bindings::{CloseHandle, HANDLE};

const TH32CS_SNAPPROCESS: u32 = 0x2;
const TH32CS_SNAPMODULE: u32 = 0x8;
const TH32CS_SNAPMODULE32: u32 = 0x10;

/// Modules whose presence in explorer.exe means Fences manages the desktop.
pub const DESKTOP_MODULES: [&str; 2] = ["DesktopDock64.dll", "Detour64.dll"];
/// Fences processes (its configuration window, its helpers).
pub const PROCESSES: [&str; 3] = ["Fences.exe", "FencesTaskbarItem.exe", "SdDisplay.exe"];

#[repr(C)]
struct ProcessEntry32W {
    size: u32,
    usage: u32,
    pid: u32,
    heap: usize,
    module: u32,
    threads: u32,
    parent: u32,
    priority: i32,
    flags: u32,
    exe: [u16; 260],
}

#[repr(C)]
struct ModuleEntry32W {
    size: u32,
    module_id: u32,
    pid: u32,
    global_usage: u32,
    proc_usage: u32,
    base: *mut u8,
    base_size: u32,
    handle: *mut core::ffi::c_void,
    module: [u16; 256],
    path: [u16; 260],
}

windows_core::link!("kernel32.dll" "system" fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> HANDLE);
windows_core::link!("kernel32.dll" "system" fn Process32FirstW(snapshot: HANDLE, entry: *mut ProcessEntry32W) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn Process32NextW(snapshot: HANDLE, entry: *mut ProcessEntry32W) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn Module32FirstW(snapshot: HANDLE, entry: *mut ModuleEntry32W) -> windows_core::BOOL);
windows_core::link!("kernel32.dll" "system" fn Module32NextW(snapshot: HANDLE, entry: *mut ModuleEntry32W) -> windows_core::BOOL);

/// What was found; empty lists = Fences is not active.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FencesPresence {
    /// `explorer.exe (pid 1234): DesktopDock64.dll`
    pub explorer_modules: Vec<String>,
    /// `Fences.exe (pid 1234)`
    pub processes: Vec<String>,
    /// Explorer processes whose modules could not be listed.
    pub unreadable: Vec<u32>,
}

impl FencesPresence {
    /// Fences draws or manages the desktop right now.
    pub fn active(&self) -> bool {
        !self.explorer_modules.is_empty() || !self.processes.is_empty()
    }
}

fn text(buf: &[u16]) -> String {
    let n = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..n])
}

fn valid(h: HANDLE) -> bool {
    !h.0.is_null() && h.0 as isize != -1
}

/// Looks for Fences in the running processes and in every explorer.exe.
pub fn detect() -> FencesPresence {
    let mut found = FencesPresence::default();
    let mut explorers = Vec::new();
    // SAFETY: plain FFI; the entry outlives the calls and the snapshot is closed below.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if !valid(snap) {
            return found;
        }
        let mut e: ProcessEntry32W = std::mem::zeroed();
        e.size = size_of::<ProcessEntry32W>() as u32;
        let mut more = Process32FirstW(snap, &mut e).as_bool();
        while more {
            let name = text(&e.exe);
            if name.eq_ignore_ascii_case("explorer.exe") {
                explorers.push(e.pid);
            } else if PROCESSES.iter().any(|p| p.eq_ignore_ascii_case(&name)) {
                found.processes.push(format!("{name} (pid {})", e.pid));
            }
            more = Process32NextW(snap, &mut e).as_bool();
        }
        let _ = CloseHandle(snap);
    }
    for pid in explorers {
        // SAFETY: as above.
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid);
            if !valid(snap) {
                found.unreadable.push(pid);
                continue;
            }
            let mut m: ModuleEntry32W = std::mem::zeroed();
            m.size = size_of::<ModuleEntry32W>() as u32;
            let mut more = Module32FirstW(snap, &mut m).as_bool();
            while more {
                let name = text(&m.module);
                if DESKTOP_MODULES
                    .iter()
                    .any(|d| d.eq_ignore_ascii_case(&name))
                {
                    found
                        .explorer_modules
                        .push(format!("explorer.exe (pid {pid}): {name}"));
                }
                more = Module32NextW(snap, &mut m).as_bool();
            }
            let _ = CloseHandle(snap);
        }
    }
    found
}
