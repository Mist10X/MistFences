//! A right-button drag on the bare desktop, as in Fences: the rectangle it draws offers a new
//! fence there instead of Explorer's desktop menu.
//!
//! Explorer opens its menu when the right button comes up, and it receives the button before
//! anyone else can see it, so the press is held back with a low-level mouse hook (its own
//! thread, so a busy UI thread never delays the mouse). Only a right press on the bare desktop
//! while Windows' desktop icons are hidden is held; everything else passes untouched. A press
//! released without moving is replayed with `SendInput`, so Explorer opens its menu as usual;
//! one dragged past the drag threshold is a rectangle, reported to `notify` and never seen by
//! Explorer.

use crate::bindings::*;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};

/// `wparam` of the message posted to `notify`; `lparam` packs the screen point
/// (`x` low word, `y` high word, both signed).
pub const RDRAG_PRESSED: usize = 1;
pub const RDRAG_MOVED: usize = 2;
pub const RDRAG_RELEASED: usize = 3;
pub const RDRAG_CANCELLED: usize = 4;

/// Marks the replayed click so the hook lets it through.
const REPLAY_MARK: usize = 0x4D46_5244; // "MFRD"
/// Thread message to the hook thread: replay a right click where the pointer is.
const MSG_REPLAY: u32 = WM_APP as u32 + 1;

static NOTIFY: AtomicIsize = AtomicIsize::new(0);
static NOTIFY_MSG: AtomicU32 = AtomicU32::new(0);
static HOOK_THREAD: AtomicU32 = AtomicU32::new(0);
/// The newest pointer position of a rectangle being drawn, and whether `notify` still has a
/// move to read: moves are coalesced, so a busy UI thread never draws stale ones behind the
/// pointer.
static LAST_MOVE: AtomicIsize = AtomicIsize::new(0);
static MOVE_PENDING: AtomicBool = AtomicBool::new(false);
static STATE: Mutex<Press> = Mutex::new(Press {
    origin: None,
    live: false,
    cancelled: false,
});

struct Press {
    /// A right press on the bare desktop is held back, from here.
    origin: Option<POINT>,
    /// The pointer left the drag threshold: a rectangle is being drawn.
    live: bool,
    /// The left button went down meanwhile: no rectangle and no menu.
    cancelled: bool,
}

/// The hook thread; dropping it removes the hook.
pub struct RightDragWatch {
    thread: Option<std::thread::JoinHandle<()>>,
    thread_id: u32,
}

impl Drop for RightDragWatch {
    fn drop(&mut self) {
        // SAFETY: plain FFI call; the thread ends its loop on WM_QUIT and removes the hook.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT as u32, WPARAM(0), LPARAM(0));
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        HOOK_THREAD.store(0, Ordering::SeqCst);
    }
}

/// Starts watching; events go to `notify` as `message` (see `RDRAG_*`). None in the preview
/// (the real desktop is not ours there) or when Windows refuses the hook.
pub fn watch(notify: HWND, message: u32) -> Option<RightDragWatch> {
    if !crate::safety::allow_system_change("watch right-button drags on the desktop") {
        return None;
    }
    NOTIFY.store(notify.0 as isize, Ordering::SeqCst);
    NOTIFY_MSG.store(message, Ordering::SeqCst);
    let (tx, rx) = std::sync::mpsc::channel();
    let thread = std::thread::Builder::new()
        .name("rdrag-hook".into())
        .spawn(move || {
            // SAFETY: the hook is installed and removed on this thread, which pumps messages
            // for it until WM_QUIT.
            unsafe {
                let module = GetModuleHandleW(windows_core::PCWSTR::null());
                let hook =
                    SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), Some(HINSTANCE(module.0)), 0);
                let ok = !hook.0.is_null();
                let _ = tx.send(ok.then(|| GetCurrentThreadId()));
                if !ok {
                    return;
                }
                let mut m = MSG::default();
                while GetMessageW(&mut m, None, 0, 0).0 > 0 {
                    if m.message == MSG_REPLAY {
                        replay_right_click();
                    }
                }
                let _ = UnhookWindowsHookEx(hook);
            }
        })
        .ok()?;
    match rx.recv().ok().flatten() {
        Some(thread_id) => {
            HOOK_THREAD.store(thread_id, Ordering::SeqCst);
            tracing::info!("right-button drag on the desktop watched");
            Some(RightDragWatch {
                thread: Some(thread),
                thread_id,
            })
        }
        None => {
            tracing::warn!("low-level mouse hook refused");
            let _ = thread.join();
            None
        }
    }
}

/// Packs a screen point into an `lparam` (signed 16-bit words, as `WM_*` mouse messages do).
pub fn pack_point(pt: POINT) -> isize {
    ((pt.x as u16 as u32) | ((pt.y as u16 as u32) << 16)) as i32 as isize
}

/// The newest position of the rectangle being drawn (read on `RDRAG_MOVED`; later moves post
/// again).
pub fn latest_move() -> POINT {
    MOVE_PENDING.store(false, Ordering::SeqCst);
    unpack_point(LAST_MOVE.load(Ordering::SeqCst))
}

pub fn unpack_point(lparam: isize) -> POINT {
    POINT {
        x: (lparam & 0xFFFF) as u16 as i16 as i32,
        y: ((lparam >> 16) & 0xFFFF) as u16 as i16 as i32,
    }
}

fn post(kind: usize, pt: POINT) {
    let hwnd = HWND(NOTIFY.load(Ordering::SeqCst) as *mut core::ffi::c_void);
    // SAFETY: plain FFI call; a stale handle only fails.
    unsafe {
        let _ = PostMessageW(
            Some(hwnd),
            NOTIFY_MSG.load(Ordering::SeqCst),
            WPARAM(kind),
            LPARAM(pack_point(pt)),
        );
    }
}

/// Bare desktop under `pt`: Explorer's desktop window (not a fence, not a free icon, whose
/// pixels hit-test as our layer) while Windows' own icons are hidden.
fn bare_desktop_at(pt: POINT) -> bool {
    let hit = crate::desktop::window_from_point(pt.x, pt.y);
    if hit.0.is_null() {
        return false;
    }
    let root = crate::desktop::root_ancestor(hit);
    let cls = crate::desktop::class_name(root);
    (cls == "Progman" || cls == "WorkerW")
        && crate::desktop::window_pid(root)
            == crate::desktop::window_pid(crate::desktop::shell_window())
        && crate::shell_icons::desktop_icons_hidden()
}

fn beyond_drag_threshold(a: POINT, b: POINT) -> bool {
    // SAFETY: plain FFI calls.
    let (tx, ty) = unsafe { (GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG)) };
    (a.x - b.x).abs() > tx || (a.y - b.y).abs() > ty
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        // SAFETY: for WH_MOUSE_LL, lparam points to an MSLLHOOKSTRUCT for this call.
        let info = unsafe { &*(lparam.0 as *const MSLLHOOKSTRUCT) };
        if info.dwExtraInfo != REPLAY_MARK && on_mouse(wparam.0 as u32, info.pt) {
            return LRESULT(1);
        }
    }
    // SAFETY: plain FFI call with the arguments received.
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

/// One mouse event; true when it is held back from everyone else.
fn on_mouse(m: u32, pt: POINT) -> bool {
    let Ok(mut s) = STATE.lock() else {
        return false;
    };
    match m {
        x if x == WM_RBUTTONDOWN as u32 => {
            if bare_desktop_at(pt) {
                MOVE_PENDING.store(false, Ordering::SeqCst);
                *s = Press {
                    origin: Some(pt),
                    live: false,
                    cancelled: false,
                };
                post(RDRAG_PRESSED, pt);
                true
            } else {
                s.origin = None;
                false
            }
        }
        x if x == WM_MOUSEMOVE as u32 => {
            if let Some(origin) = s.origin
                && !s.cancelled
            {
                if !s.live && beyond_drag_threshold(origin, pt) {
                    s.live = true;
                }
                if s.live {
                    LAST_MOVE.store(pack_point(pt), Ordering::SeqCst);
                    if !MOVE_PENDING.swap(true, Ordering::SeqCst) {
                        post(RDRAG_MOVED, pt);
                    }
                }
            }
            false
        }
        x if x == WM_LBUTTONDOWN as u32 => {
            if s.origin.is_some() && !s.cancelled {
                s.cancelled = true;
                post(RDRAG_CANCELLED, pt);
            }
            false
        }
        x if x == WM_RBUTTONUP as u32 => {
            let Some(origin) = s.origin.take() else {
                return false;
            };
            if s.cancelled {
            } else if s.live || beyond_drag_threshold(origin, pt) {
                post(RDRAG_RELEASED, pt);
            } else {
                // A plain right click: Explorer gets it (and opens its menu) after all.
                post(RDRAG_CANCELLED, pt);
                // SAFETY: plain FFI call to this hook's own thread.
                unsafe {
                    let _ = PostThreadMessageW(
                        HOOK_THREAD.load(Ordering::SeqCst),
                        MSG_REPLAY,
                        WPARAM(0),
                        LPARAM(0),
                    );
                }
            }
            true
        }
        _ => false,
    }
}

/// The held-back right click, sent again where the pointer is, marked so the hook passes it.
fn replay_right_click() {
    let input = |flags: i32| INPUT {
        r#type: INPUT_MOUSE as u32,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags as u32,
                dwExtraInfo: REPLAY_MARK,
                ..Default::default()
            },
        },
    };
    let inputs = [input(MOUSEEVENTF_RIGHTDOWN), input(MOUSEEVENTF_RIGHTUP)];
    // SAFETY: two well-formed mouse inputs.
    unsafe {
        let _ = SendInput(&inputs, size_of::<INPUT>() as i32);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_survive_packing() {
        for pt in [
            POINT { x: 0, y: 0 },
            POINT { x: 2559, y: 1391 },
            POINT { x: -1920, y: -40 },
            POINT { x: 4479, y: 1079 },
        ] {
            let back = unpack_point(pack_point(pt));
            assert_eq!((back.x, back.y), (pt.x, pt.y));
        }
    }
}
