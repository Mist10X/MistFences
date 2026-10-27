//! Reading and toggling the "Show desktop icons" shell setting (plan §5.3).
//!
//! Reading uses `SHGetSetSettings(SSF_HIDEICONS)`. Toggling sends the same `WM_COMMAND 0x7402`
//! that Explorer's "View → Show desktop icons" menu item sends to `SHELLDLL_DefView`, so Explorer
//! persists the state itself (registry `HideIcons`) and the menu stays in sync.

use crate::bindings::*;
use crate::wide::to_wide;
use windows_core::{Interface, PCWSTR};

const HIDE_ICONS_BIT: u32 = 1 << 12; // SHELLSTATE::fHideIcons
const CMD_TOGGLE_DESKTOP_ICONS: usize = 0x7402;
const LVM_GETSELECTEDCOUNT: u32 = 0x1000 + 50; // LVM_FIRST + 50
const LVM_GETHOTITEM: u32 = 0x1000 + 61; // LVM_FIRST + 61
const LVM_GETITEMCOUNT: u32 = 0x1000 + 4; // LVM_FIRST + 4
const LVM_GETITEMSTATE: u32 = 0x1000 + 44; // LVM_FIRST + 44
const LVIS_DROPHILITED: isize = 0x0008;

/// True when the shell currently hides desktop icons.
pub fn desktop_icons_hidden() -> bool {
    let mut state = SHELLSTATEW::default();
    // SAFETY: SHELLSTATEA and SHELLSTATEW share the same layout; `state` outlives the call.
    unsafe {
        SHGetSetSettings(
            Some((&mut state as *mut SHELLSTATEW).cast()),
            SSF_HIDEICONS as u32,
            false,
        );
    }
    (state._bitfield1.0 as u32) & HIDE_ICONS_BIT != 0
}

/// Sends the toggle command to the desktop view (falls back to `Progman` when `def_view` is
/// null). Returns false if Explorer did not answer within the timeout.
pub fn toggle_desktop_icons(def_view: HWND, fallback: HWND) -> bool {
    if !crate::safety::allow_system_change("toggle the Windows desktop icons") {
        return false;
    }
    let target = if def_view.0.is_null() {
        fallback
    } else {
        def_view
    };
    if target.0.is_null() {
        return false;
    }
    let mut result: usize = 0;
    // SAFETY: plain FFI call with a timeout; Explorer owns the target window.
    let ok = unsafe {
        SendMessageTimeoutW(
            target,
            WM_COMMAND as u32,
            WPARAM(CMD_TOGGLE_DESKTOP_ICONS),
            LPARAM(0),
            SMTO_ABORTIFHUNG as u32,
            1000,
            Some(&mut result),
        )
    };
    ok.0 != 0
}

/// How many desktop icons are selected in Explorer's list view under `def_view`; None when
/// there is no view or Explorer did not answer.
pub fn desktop_selected_count(def_view: HWND) -> Option<usize> {
    if def_view.0.is_null() {
        return None;
    }
    let class = to_wide("SysListView32");
    // SAFETY: the string outlives the call.
    let list =
        unsafe { FindWindowExW(Some(def_view), None, PCWSTR(class.as_ptr()), PCWSTR::null()) };
    if list.0.is_null() {
        return None;
    }
    let mut count: usize = 0;
    // SAFETY: plain FFI call with a timeout; Explorer owns the list view.
    let ok = unsafe {
        SendMessageTimeoutW(
            list,
            LVM_GETSELECTEDCOUNT,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG as u32,
            200,
            Some(&mut count),
        )
    };
    (ok.0 != 0).then_some(count)
}

/// True when the pointer is on one of Windows' own desktop icons: `window` (what
/// `WindowFromPoint` found under it) is Explorer's desktop list view, and the icon it tracks for
/// its hover highlight is set. False for its empty space, any other window, or no answer.
pub fn desktop_icon_hovered(window: HWND) -> bool {
    if crate::desktop::class_name(window) != "SysListView32" {
        return false;
    }
    let mut item = usize::MAX;
    // SAFETY: plain FFI call with a timeout; Explorer owns the list view.
    unsafe {
        SendMessageTimeoutW(
            window,
            LVM_GETHOTITEM,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG as u32,
            200,
            Some(&mut item),
        );
    }
    (item as isize) >= 0
}

/// True while a drag is over one of Windows' own desktop icons that takes the drop (a folder,
/// a program, the Recycle Bin): `window` is Explorer's desktop list view, which marks that icon
/// drop-highlighted. False for its empty space, any other window, or no answer. Each icon is
/// asked on its own: on build 26300 `LVM_GETNEXTITEM` finds no drop-highlighted icon while
/// `LVM_GETITEMSTATE` shows it.
pub fn desktop_icon_drop_target(window: HWND) -> bool {
    if crate::desktop::class_name(window) != "SysListView32" {
        return false;
    }
    let ask = |msg: u32, wparam: usize, lparam: isize| {
        let mut out = 0usize;
        // SAFETY: plain FFI call with a timeout; Explorer owns the list view.
        unsafe {
            SendMessageTimeoutW(
                window,
                msg,
                WPARAM(wparam),
                LPARAM(lparam),
                SMTO_ABORTIFHUNG as u32,
                200,
                Some(&mut out),
            );
        }
        out
    };
    let count = ask(LVM_GETITEMCOUNT, 0, 0).min(4096);
    (0..count).any(|i| ask(LVM_GETITEMSTATE, i, LVIS_DROPHILITED) & LVIS_DROPHILITED as usize != 0)
}

/// Explorer's desktop grid as its list view reports it (read only; nothing is changed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExplorerGrid {
    /// Icon spacing in pixels at the system DPI (`LVM_GETITEMSPACING`): 75 x 99 with medium
    /// icons at 100 %.
    pub spacing: (i32, i32),
    /// "Align icons to grid" (`LVS_EX_SNAPTOGRID`).
    pub snap_to_grid: bool,
    /// Where the grid starts in a work area, in pixels: where most of Explorer's own icons sit
    /// modulo the spacing (`IFolderView::GetItemPosition`). None when it has no icons to ask.
    pub origin: Option<(i32, i32)>,
}

/// Reads Explorer's desktop grid from the list view under `def_view`; None when there is none
/// or Explorer does not answer.
pub fn explorer_grid(def_view: HWND) -> Option<ExplorerGrid> {
    const LVM_GETITEMSPACING: u32 = 0x1000 + 51;
    const LVM_GETEXTENDEDLISTVIEWSTYLE: u32 = 0x1000 + 55;
    const LVS_EX_SNAPTOGRID: usize = 0x0008_0000;
    if def_view.0.is_null() {
        return None;
    }
    let class = to_wide("SysListView32");
    // SAFETY: the string outlives the call.
    let list =
        unsafe { FindWindowExW(Some(def_view), None, PCWSTR(class.as_ptr()), PCWSTR::null()) };
    if list.0.is_null() {
        return None;
    }
    let ask = |msg: u32| -> Option<usize> {
        let mut out = 0usize;
        // SAFETY: plain FFI call with a timeout; no pointers cross the process boundary.
        let ok = unsafe {
            SendMessageTimeoutW(
                list,
                msg,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG as u32,
                500,
                Some(&mut out),
            )
        };
        (ok.0 != 0).then_some(out)
    };
    let spacing = ask(LVM_GETITEMSPACING)?;
    let (cx, cy) = ((spacing & 0xFFFF) as i32, ((spacing >> 16) & 0xFFFF) as i32);
    if cx <= 0 || cy <= 0 {
        return None;
    }
    let style = ask(LVM_GETEXTENDEDLISTVIEWSTYLE).unwrap_or(0);
    let areas: Vec<RECT> = crate::monitors::enumerate()
        .into_iter()
        .map(|m| m.work_area)
        .collect();
    // An item's position is its icon picture's top-left in the list view's own coordinates; its
    // icon box (`LVM_GETITEMRECT`, LVIR_ICON: one pixel narrower than a cell) centres the
    // picture, and on Windows 11 the view starts 3 px (at 96 DPI) above the window. Measured in
    // Windows Sandbox: position (13, 2) for the box at (0, 5) with 48 px icons, 75 x 103 cells.
    let (positions, icon) = explorer_icon_positions(list);
    let dpi = crate::monitors::system_dpi().max(96) as i32;
    let dx = (cx - 1 - icon.unwrap_or(48 * dpi / 96)).max(0) / 2;
    let dy = (3 * dpi + 48) / 96;
    let in_areas: Vec<(i32, i32)> = positions
        .into_iter()
        .filter_map(|p| {
            let (x, y) = (p.x - dx, p.y + dy);
            areas
                .iter()
                .find(|a| x >= a.left && x < a.right && y >= a.top && y < a.bottom)
                .map(|a| (x - a.left, y - a.top))
        })
        .collect();
    Some(ExplorerGrid {
        spacing: (cx, cy),
        snap_to_grid: style & LVS_EX_SNAPTOGRID != 0,
        origin: grid_origin(&in_areas, (cx, cy)),
    })
}

/// The most common place of `points` (relative to their work areas) modulo `spacing`: the
/// grid origin they were arranged on. Ties go to the smaller offset.
pub fn grid_origin(points: &[(i32, i32)], spacing: (i32, i32)) -> Option<(i32, i32)> {
    let mut counts: std::collections::BTreeMap<(i32, i32), usize> = Default::default();
    for &(x, y) in points {
        *counts
            .entry((x.rem_euclid(spacing.0), y.rem_euclid(spacing.1)))
            .or_default() += 1;
    }
    let best = counts.values().copied().max()?;
    counts.into_iter().find(|(_, n)| *n == best).map(|(o, _)| o)
}

/// Explorer's own desktop icons, in screen pixels, as its folder view places them, and its icon
/// size (read only): `IShellWindows::FindWindowSW(SWC_DESKTOP)` → `IShellBrowser` →
/// `IFolderView2`. At most 64 items are asked (enough to find the grid; each is a call into
/// Explorer). Empty when the desktop's view does not answer. COM must be initialised on the
/// calling thread.
pub fn explorer_icon_positions(list_view: HWND) -> (Vec<POINT>, Option<i32>) {
    let view = || -> windows_core::Result<IFolderView2> {
        // SAFETY: COM calls on interfaces this function owns; the VARIANTs outlive them.
        unsafe {
            let windows: IShellWindows = CoCreateInstance(&ShellWindows, None, CLSCTX_ALL as u32)?;
            let mut at = VARIANT::default();
            (*at.Anonymous.Anonymous).vt = VARTYPE(3); // VT_I4
            (*at.Anonymous.Anonymous).Anonymous.lVal = CSIDL_DESKTOP;
            let root = VARIANT::default();
            let mut hwnd = 0i32;
            let desktop =
                windows.FindWindowSW(&at, &root, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH)?;
            let services: IServiceProvider = desktop.cast()?;
            let browser: IShellBrowser = services.QueryService(&SID_STopLevelBrowser)?;
            browser.QueryActiveShellView()?.cast()
        }
    };
    let view = match view() {
        Ok(v) => v,
        Err(e) => {
            tracing::debug!(error = %e, "desktop folder view unavailable");
            return (Vec::new(), None);
        }
    };
    // SAFETY: as above; each item ID list is the view's copy, freed after use.
    unsafe {
        let mut mode = FOLDERVIEWMODE::default();
        let mut size = 0i32;
        let icon = view
            .GetViewModeAndIconSize(&mut mode, &mut size)
            .is_ok()
            .then_some(size)
            .filter(|s| (16..=256).contains(s));
        let count = view.ItemCount(SVGIO_ALLVIEW).unwrap_or(0).clamp(0, 64);
        let positions = (0..count)
            .filter_map(|i| {
                let pidl = view.Item(i).ok()?;
                let at = view.GetItemPosition(pidl);
                CoTaskMemFree(pidl as _);
                let mut p = at.ok()?;
                let _ = ClientToScreen(list_view, &mut p);
                Some(p)
            })
            .collect();
        (positions, icon)
    }
}

/// Sets the hidden state, toggling only when it differs. Returns the state read back afterwards.
pub fn set_desktop_icons_hidden(hidden: bool, def_view: HWND, fallback: HWND) -> Option<bool> {
    if desktop_icons_hidden() != hidden && !toggle_desktop_icons(def_view, fallback) {
        return None;
    }
    Some(desktop_icons_hidden())
}

#[cfg(test)]
mod tests {
    use super::grid_origin;

    #[test]
    fn grid_origin_is_where_most_icons_sit() {
        // Explorer at 100 % in Windows 11: icons at y = 5, 108, 211 with 75 x 103 spacing; one
        // icon dragged off the grid does not move the origin.
        let icons = [(0, 5), (0, 108), (0, 211), (75, 5), (311, 47)];
        assert_eq!(grid_origin(&icons, (75, 103)), Some((0, 5)));
        assert_eq!(grid_origin(&[], (75, 103)), None);
    }
}
