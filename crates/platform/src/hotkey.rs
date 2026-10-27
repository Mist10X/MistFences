//! Global hotkeys (`RegisterHotKey`): the owning window receives `WM_HOTKEY` with the id in
//! `wParam`.

use crate::bindings::*;

/// Modifier bits for [`register`].
pub mod mods {
    use crate::bindings as b;
    pub const ALT: u32 = b::MOD_ALT as u32;
    pub const CONTROL: u32 = b::MOD_CONTROL as u32;
    pub const SHIFT: u32 = b::MOD_SHIFT as u32;
    pub const WIN: u32 = b::MOD_WIN as u32;
    /// Do not repeat while the keys stay held.
    pub const NOREPEAT: u32 = b::MOD_NOREPEAT as u32;
}

pub const VK_SPACE: u32 = crate::bindings::VK_SPACE as u32;

/// Registers `id` on `hwnd`. Returns false when the combination is taken by another process
/// (the caller should tell the user or pick a fallback).
pub fn register(hwnd: HWND, id: i32, modifiers: u32, vk: u32) -> bool {
    // A global shortcut takes the key combination from every other program (Fences'
    // shortcuts included). The preview keeps only the bare Esc that cancels a fence move
    // while the mouse holds it.
    const VK_ESCAPE: u32 = 0x1B;
    let move_cancel = vk == VK_ESCAPE && modifiers & 0x000F == 0;
    if !move_cancel && !crate::safety::allow_system_change("register a global shortcut") {
        return false;
    }
    // SAFETY: plain FFI call; `hwnd` belongs to this thread.
    unsafe { RegisterHotKey(Some(hwnd), id, modifiers, vk).as_bool() }
}

pub fn unregister(hwnd: HWND, id: i32) {
    // SAFETY: plain FFI call.
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), id);
    }
}
