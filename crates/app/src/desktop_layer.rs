//! Free desktop icons on screen (MistFences). One window per monitor that holds free icons,
//! just above Explorer's desktop and below the fences (`DesktopAnchor::register_layer`). Each
//! is a `WS_EX_LAYERED` window presented with `UpdateLayeredWindowIndirect` and per-pixel
//! alpha: Windows hit-tests such a window by its pixels, so everything but the icons and
//! their labels (backed at alpha 1, invisible) is click-through to the desktop beneath, its
//! context menu and its rubber band included. No hooks, no polling.
//!
//! What it looks like follows Explorer's desktop list view: its grid (`AppState::desk_grid`),
//! the icon 2 px below the cell's top and centred, the label under the icon box in the
//! icon-title font with the system's smoothing (ClearType when it is on), white over a dark
//! shadow, broken between words with "..." where Explorer puts it (`textmask`).
//!
//! The windows keep their bitmap: a change of hover, selection or drop highlight redraws only
//! the cells it concerns and tells the system which part changed. The icon pictures the layer
//! shows are its own (`images`), so the icon cache dropping unused entries while drawing is
//! quiet (`App::prune_icons`) never blanks them: that was the "all icons vanish for a moment
//! on hover" flicker.
//!
//! Interaction mirrors Explorer's desktop: click / Ctrl+click selects, double-click opens,
//! right-click shows the item menu, and dragging is the shell's OLE drag with the icons' own
//! picture under the cursor. Over a free folder, shortcut or the Recycle Bin a drag goes to
//! that item's own shell drop target (Explorer does the same), which performs the file
//! operation itself; anywhere else on the desktop free icons just move.

use crate::anchor::FENCE_CLASS;
use crate::commands::{Command, CommandQueue, TransferMode};
use crate::fence_window::FenceContext;
use crate::free_icons::{DESK_ICON_TOP_DIP, FreeIcon};
use crate::icons::{IconCache, Lookup};
use pecofence_core::desk::DeskGrid;
use pecofence_core::geometry::WorkArea;
use pecofence_core::{FenceId, IconKey, ItemId};
use pecofence_platform::dragdrop::{
    self, DragImage, DragPoint, DropEffect, DropHandler, DropTargetHelper, DropTargetRegistration,
    IDataObject, ItemDropTarget,
};
use pecofence_platform::layered::LayeredImage;
use pecofence_platform::textmask::{LabelText, cleartype_on, render_label};
use pecofence_platform::window::{self, ClassOptions, Window, WindowBuilder, WindowClass};
use pecofence_platform::{HWND, POINT, RECT, desktop, msg, sysparams};
use pecofence_render::Image;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::{Rc, Weak};

pub const LAYER_CLASS: &str = "MistFences.DesktopLayer";

const WS_POPUP: u32 = 0x8000_0000;
const WS_EX_LAYERED: u32 = 0x0008_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;
const HTCLIENT: isize = 1;
const HTTRANSPARENT: isize = -1;
const MA_NOACTIVATE: isize = 3;
const WM_MOUSEACTIVATE: u32 = 0x0021;
const WM_NCHITTEST: u32 = 0x0084;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_LBUTTONDBLCLK: u32 = 0x0203;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MOUSELEAVE: u32 = 0x02A3;
const WM_CAPTURECHANGED: u32 = 0x0215;
const MK_CONTROL: usize = 0x0008;
const MK_SHIFT: usize = 0x0004;
/// Label lines under an unselected icon (Explorer).
const LABEL_LINES: usize = 2;
/// Room Explorer keeps on both sides of a label's text (`SM_CXEDGE`): its lines wrap at the
/// cell less twice this.
const LABEL_MARGIN_DIP: f32 = 2.0;
/// Explorer's icon box: the picture with 2 px around it; the label starts 1 px below it.
const ICON_BOX_MARGIN_DIP: f32 = 2.0;
const LABEL_GAP_DIP: f32 = 1.0;

thread_local! {
    /// Set when a drag ended on a free item's own drop target (a file operation into a folder,
    /// the Recycle Bin, a program): the drag source then must not take the drop for a move of
    /// icons on the desktop.
    static ITEM_DROP: Cell<bool> = const { Cell::new(false) };
    /// A free icon drag is running (its OLE loop pumps messages: no second one may start).
    static DRAGGING: Cell<bool> = const { Cell::new(false) };
}

/// Whether the drag that just ended was dropped onto a free item (see `ITEM_DROP`); clears it.
pub fn take_item_drop() -> bool {
    let was = ITEM_DROP.with(|d| d.replace(false));
    tracing::debug!(was, "item drop flag taken");
    was
}

/// Where Explorer's rename box opens for a free icon (screen pixels).
pub struct RenameGeometry {
    pub cell: RECT,
    /// The top of its label.
    pub label_top: i32,
    pub dpi: u32,
    /// The icon-title font at the monitor's DPI: face, pixel height, weight.
    pub font: (String, i32, i32),
}

/// What the layer shows for one free icon.
#[derive(Clone, Debug, PartialEq)]
pub struct DeskItem {
    pub id: ItemId,
    pub label: String,
    pub path: PathBuf,
    pub icon: IconKey,
}

#[derive(PartialEq)]
struct Placed {
    item: DeskItem,
    monitor: String,
    /// The cell in screen pixels.
    cell: RECT,
    dpi: u32,
}

struct LayerWin {
    window: Window,
    monitor: String,
    /// Screen position of the bitmap's top-left pixel.
    origin: POINT,
    image: Option<LayeredImage>,
    _drop: Option<DropTargetRegistration>,
}

struct Press {
    item: ItemId,
    start: POINT,
    /// The press selected nothing new (Explorer: a click on a selected icon of several
    /// narrows the selection only on release, so the drag can take them all).
    narrow_on_release: bool,
}

type LabelKey = (ItemId, String, i32, i32, bool);

struct Shared {
    placed: Vec<Placed>,
    windows: Vec<LayerWin>,
    hover: Option<ItemId>,
    selected: Vec<ItemId>,
    /// The free item a drag is over that takes the drop (highlighted like a selection).
    drop_hover: Option<ItemId>,
    press: Option<Press>,
    /// Pictures drawn last, per item: kept while the icon cache drops or re-extracts them.
    images: HashMap<ItemId, (String, Rc<Image>)>,
    labels: HashMap<LabelKey, Rc<LabelText>>,
    font: (String, f32, i32),
    cleartype: bool,
    grid: DeskGrid,
    icon_dip: u32,
    inbox: FenceId,
    visible: bool,
    /// An icon was still being extracted at the last draw.
    pending_icons: bool,
    queue: CommandQueue,
    ctx: Rc<FenceContext>,
}

pub struct DesktopLayer {
    shared: Rc<RefCell<Shared>>,
    class: WindowClass,
}

/// The window under the pointer, its top-level class, and whether that is the desktop:
/// Explorer's desktop windows, or this layer (free icons).
pub fn pointer_target() -> (HWND, String, bool) {
    let pt = window::cursor_pos();
    let hit = desktop::window_from_point(pt.x, pt.y);
    let class = desktop::class_name(desktop::root_ancestor(hit));
    let on_desktop = matches!(
        class.as_str(),
        "Progman" | "WorkerW" | "SHELLDLL_DefView" | "SysListView32" | LAYER_CLASS
    );
    (hit, class, on_desktop)
}

/// Whether a drag source must end its drag itself at the button-up instead of handing the
/// drop to the target: over Explorer's own (hidden-icon) desktop a plain drag of our icons is
/// a move of icons, not a file operation. Over this layer the layer decides (its folders take
/// files, the rest is a move of icons), so the drop goes through.
pub fn skip_drop_at_pointer(keys: u32) -> bool {
    let (hit, class, on_desktop) = pointer_target();
    on_desktop
        && class != LAYER_CLASS
        && dragdrop::modifier_effect(keys, true) == DropEffect::Move
        && !pecofence_platform::shell_icons::desktop_icon_drop_target(hit)
}

impl DesktopLayer {
    pub fn new(ctx: Rc<FenceContext>, queue: CommandQueue) -> windows_core::Result<Self> {
        let class = WindowClass::register(
            LAYER_CLASS,
            ClassOptions {
                double_clicks: true,
                background: None,
            },
        )?;
        Ok(Self {
            shared: Rc::new(RefCell::new(Shared {
                placed: Vec::new(),
                windows: Vec::new(),
                hover: None,
                selected: Vec::new(),
                drop_hover: None,
                press: None,
                images: HashMap::new(),
                labels: HashMap::new(),
                font: label_font(),
                cleartype: cleartype_on(),
                grid: DeskGrid::default(),
                icon_dip: 48,
                inbox: FenceId::nil(),
                visible: true,
                pending_icons: false,
                queue,
                ctx,
            })),
            class,
        })
    }

    /// Shows `icons` (with what to draw for each) where they are on `grid`; creates and drops
    /// the per-monitor windows as needed and redraws them. `visible` = false hides the layer
    /// (the Windows icons show, which already include these items).
    pub fn sync(
        &self,
        icons: Vec<(FreeIcon, DeskItem)>,
        areas: &[WorkArea],
        grid: DeskGrid,
        inbox: FenceId,
        icon_dip: u32,
        visible: bool,
    ) {
        let mut changed = false;
        let monitors: Vec<String> = {
            let mut s = self.shared.borrow_mut();
            changed |= s.inbox != inbox || s.icon_dip != icon_dip || s.visible != visible;
            changed |= s.grid != grid;
            s.inbox = inbox;
            s.icon_dip = icon_dip;
            s.visible = visible;
            s.grid = grid;
            let font = label_font();
            let cleartype = cleartype_on();
            if font != s.font || cleartype != s.cleartype {
                s.font = font;
                s.cleartype = cleartype;
                s.labels.clear();
                changed = true;
            }
            let placed: Vec<Placed> = icons
                .into_iter()
                .filter_map(|(fi, item)| {
                    let work = areas.iter().find(|w| w.device_path == fi.monitor)?;
                    let sc = work.scale();
                    let left = work.left + (fi.x * sc).round() as i32;
                    let top = work.top + (fi.y * sc).round() as i32;
                    Some(Placed {
                        item,
                        monitor: fi.monitor,
                        cell: RECT {
                            left,
                            top,
                            right: left + (grid.cell_w * sc).round() as i32,
                            bottom: top + (grid.cell_h * sc).round() as i32,
                        },
                        dpi: work.dpi,
                    })
                })
                .collect();
            changed |= placed != s.placed;
            s.placed = placed;
            let ids: HashSet<ItemId> = s.placed.iter().map(|p| p.item.id).collect();
            s.selected.retain(|id| ids.contains(id));
            if s.hover.is_some_and(|h| !ids.contains(&h)) {
                s.hover = None;
            }
            s.images.retain(|id, _| ids.contains(id));
            let live: HashSet<(ItemId, String)> = s
                .placed
                .iter()
                .map(|p| (p.item.id, p.item.label.clone()))
                .collect();
            s.labels
                .retain(|(id, label, ..), _| live.contains(&(*id, label.clone())));
            let mut m: Vec<String> = s.placed.iter().map(|p| p.monitor.clone()).collect();
            m.sort();
            m.dedup();
            if !visible {
                m.clear();
            }
            m
        };
        // Windows of monitors that lost their icons go; new ones are created outside the
        // borrow (CreateWindow calls the window procedure, which borrows `shared`).
        let gone: Vec<LayerWin> = {
            let mut s = self.shared.borrow_mut();
            let (keep, gone): (Vec<_>, Vec<_>) = std::mem::take(&mut s.windows)
                .into_iter()
                .partition(|w| monitors.contains(&w.monitor));
            s.windows = keep;
            gone
        };
        let none_gone = gone.is_empty();
        for w in gone {
            self.unregister(w.window.hwnd());
        }
        let missing: Vec<String> = {
            let s = self.shared.borrow();
            monitors
                .into_iter()
                .filter(|m| !s.windows.iter().any(|w| &w.monitor == m))
                .collect()
        };
        let created = !missing.is_empty();
        changed |= created || !none_gone;
        for monitor in missing {
            match self.create_window() {
                Ok(window) => {
                    let hwnd = window.hwnd();
                    let drop = DropTargetRegistration::register(
                        hwnd,
                        Box::new(LayerDrop {
                            shared: Rc::downgrade(&self.shared),
                            hwnd,
                            helper: DropTargetHelper::new(),
                            data: None,
                            internal: Vec::new(),
                            target: None,
                            target_folder: None,
                            effect: DropEffect::None,
                            key_state: 0,
                            allowed: 0,
                        }),
                    )
                    .map_err(|e| tracing::warn!(error = %e, "free icon drop target unavailable"))
                    .ok();
                    self.shared.borrow_mut().windows.push(LayerWin {
                        window,
                        monitor,
                        origin: POINT { x: 0, y: 0 },
                        image: None,
                        _drop: drop,
                    });
                    if let Some(a) = self.shared.borrow().ctx.anchor.borrow_mut().as_mut() {
                        a.register_layer(hwnd);
                    }
                }
                Err(e) => tracing::error!(error = %e, "desktop layer window failed"),
            }
        }
        // Nothing new to show (a sync after an unrelated change): the layer stays as it is.
        if changed {
            redraw_all(&self.shared);
        }
        if created && let Some(a) = self.shared.borrow().ctx.anchor.borrow_mut().as_mut() {
            a.reanchor("desktop layer window created");
        }
    }

    fn unregister(&self, hwnd: HWND) {
        if let Some(a) = self.shared.borrow().ctx.anchor.borrow_mut().as_mut() {
            a.unregister_fence(hwnd);
        }
    }

    fn create_window(&self) -> windows_core::Result<Window> {
        let shared = Rc::downgrade(&self.shared);
        WindowBuilder::new(&self.class)
            .title("MistFences desktop icons")
            .style(WS_POPUP)
            .ex_style(WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)
            .bounds(0, 0, 1, 1)
            .create(Box::new(move |hwnd, m, w, l| {
                let shared = shared.upgrade()?;
                on_message(&shared, hwnd, m, w, l)
            }))
    }

    /// Redraws the icons that were still being extracted at the last draw.
    pub fn redraw_if_pending_icons(&self) {
        let pending = self.shared.borrow().pending_icons;
        if pending {
            let ids: Vec<ItemId> = self
                .shared
                .borrow()
                .placed
                .iter()
                .map(|p| p.item.id)
                .collect();
            redraw_items(&self.shared, &ids);
        }
    }

    /// `IconCache::draw_key`s of the pictures on screen: kept by the icon cache's pruning.
    pub fn needed_icons(&self, icons: &IconCache, out: &mut HashSet<String>) {
        let s = self.shared.borrow();
        for p in &s.placed {
            let px = (s.icon_dip as f32 * p.dpi as f32 / 96.0).round() as u32;
            out.insert(icons.draw_key(&p.item.icon, px));
        }
    }

    pub fn clear_selection(&self) {
        let changed = std::mem::take(&mut self.shared.borrow_mut().selected);
        redraw_items(&self.shared, &changed);
    }

    /// A desktop rubber band: selects the free icons it touches (on top of `base`). Returns how
    /// many are selected now.
    pub fn select_in_rect(&self, rect: RECT, base: &[ItemId]) -> usize {
        let (before, after) = {
            let mut s = self.shared.borrow_mut();
            let mut sel: Vec<ItemId> = base.to_vec();
            for p in &s.placed {
                let c = p.cell;
                let touches = c.left < rect.right
                    && rect.left < c.right
                    && c.top < rect.bottom
                    && rect.top < c.bottom;
                if touches && !sel.contains(&p.item.id) {
                    sel.push(p.item.id);
                }
            }
            let before = std::mem::replace(&mut s.selected, sel.clone());
            (before, sel)
        };
        if before != after {
            let mut changed = before;
            changed.extend(after.iter().copied());
            redraw_items(&self.shared, &changed);
        }
        after.len()
    }

    pub fn selection(&self) -> Vec<ItemId> {
        self.shared.borrow().selected.clone()
    }

    /// After a rectangle selected free icons on Explorer's desktop: the keyboard comes here,
    /// so Delete and F2 act on them (Explorer's desktop has no icons of its own to act on).
    pub fn take_keyboard_for_selection(&self) {
        let hwnd = {
            let s = self.shared.borrow();
            let Some(first) = s.selected.first() else {
                return;
            };
            let Some(monitor) = s
                .placed
                .iter()
                .find(|p| p.item.id == *first)
                .map(|p| &p.monitor)
            else {
                return;
            };
            let Some(w) = s.windows.iter().find(|w| &w.monitor == monitor) else {
                return;
            };
            w.window.hwnd()
        };
        take_keyboard(hwnd);
    }

    /// Keeps `items` selected (after they were moved, as Explorer does).
    pub fn select(&self, items: &[ItemId]) {
        let before = std::mem::replace(&mut self.shared.borrow_mut().selected, items.to_vec());
        let mut changed = before;
        changed.extend(items.iter().copied());
        redraw_items(&self.shared, &changed);
    }

    /// Where a free icon's rename box goes (inline rename).
    pub fn rename_geometry(&self, item: ItemId) -> Option<RenameGeometry> {
        let s = self.shared.borrow();
        let p = s.placed.iter().find(|p| p.item.id == item)?;
        let sc = p.dpi as f32 / 96.0;
        Some(RenameGeometry {
            cell: p.cell,
            label_top: p.cell.top + (label_top_dip(&s) * sc).round() as i32,
            dpi: p.dpi,
            font: (s.font.0.clone(), (s.font.1 * sc).round() as i32, s.font.2),
        })
    }

    /// The cell of a free icon in screen pixels (`None` while it is not shown).
    pub fn cell(&self, item: ItemId) -> Option<RECT> {
        let s = self.shared.borrow();
        if !s.visible {
            return None;
        }
        s.placed.iter().find(|p| p.item.id == item).map(|p| p.cell)
    }

    pub fn hwnds(&self) -> Vec<HWND> {
        self.shared
            .borrow()
            .windows
            .iter()
            .map(|w| w.window.hwnd())
            .collect()
    }
}

impl Drop for DesktopLayer {
    fn drop(&mut self) {
        let hwnds = self.hwnds();
        for h in hwnds {
            self.unregister(h);
        }
        self.shared.borrow_mut().windows.clear();
    }
}

fn label_font() -> (String, f32, i32) {
    sysparams::icon_title_font()
        .map(|f| (f.family, f.size_dip, f.weight))
        .unwrap_or_else(|| ("Segoe UI".to_string(), 12.0, 400))
}

/// Where the label starts below the cell's top, in DIPs (Explorer: under the icon box).
fn label_top_dip(s: &Shared) -> f32 {
    DESK_ICON_TOP_DIP - ICON_BOX_MARGIN_DIP
        + s.icon_dip as f32
        + 2.0 * ICON_BOX_MARGIN_DIP
        + LABEL_GAP_DIP
}

/// The parts of a cell that take the mouse and get the highlight, in screen pixels: the icon
/// box and the label's lines (Explorer's item: the empty rest of the cell is the desktop's).
fn hit_rects(s: &Shared, p: &Placed) -> (RECT, Option<RECT>) {
    let sc = p.dpi as f32 / 96.0;
    let c = p.cell;
    let w = c.right - c.left;
    let box_w = w - 1;
    let icon_box = RECT {
        left: c.left,
        top: c.top + ((DESK_ICON_TOP_DIP - ICON_BOX_MARGIN_DIP) * sc).round() as i32,
        right: c.left + box_w,
        bottom: c.top
            + ((DESK_ICON_TOP_DIP + ICON_BOX_MARGIN_DIP + s.icon_dip as f32) * sc).round() as i32,
    };
    // The label's area starts right under the icon box: the 1 px between them must not let a
    // click through to the desktop (Windows 11 selects icon and label as one block).
    let label = label_for(s, p).map(|l| {
        let top = c.top + (label_top_dip(s) * sc).round() as i32;
        let (lo, hi) = lit_span(&l);
        RECT {
            left: (c.left + lo - 1).max(c.left),
            top: icon_box.bottom.min(top),
            right: (c.left + hi + 1).min(c.right),
            bottom: (top + l.height + 1).min(c.bottom),
        }
    });
    (icon_box, label)
}

/// The columns a rendered label covers (first lit, one past the last lit), across its lines.
fn lit_span(l: &LabelText) -> (i32, i32) {
    let mut lo = l.width;
    let mut hi = 0;
    for y in 0..l.height {
        for x in 0..l.width {
            if l.coverage[(y * l.width + x) as usize]
                .iter()
                .any(|v| *v > 0)
            {
                lo = lo.min(x);
                hi = hi.max(x + 1);
            }
        }
    }
    if hi > lo {
        (lo, hi)
    } else {
        (l.width / 2, l.width / 2)
    }
}

fn label_for(s: &Shared, p: &Placed) -> Option<Rc<LabelText>> {
    let sc = p.dpi as f32 / 96.0;
    let px = (s.font.1 * sc).round() as i32;
    let w = p.cell.right - p.cell.left;
    s.labels
        .get(&(p.item.id, p.item.label.clone(), px, w, s.cleartype))
        .cloned()
}

fn ensure_label(s: &mut Shared, i: usize) -> Option<Rc<LabelText>> {
    let p = &s.placed[i];
    let sc = p.dpi as f32 / 96.0;
    let px = (s.font.1 * sc).round() as i32;
    let w = p.cell.right - p.cell.left;
    let key = (p.item.id, p.item.label.clone(), px, w, s.cleartype);
    if let Some(l) = s.labels.get(&key) {
        return Some(l.clone());
    }
    let label = Rc::new(render_label(
        &p.item.label,
        &s.font.0,
        px,
        s.font.2,
        w,
        (LABEL_MARGIN_DIP * sc).round() as i32,
        LABEL_LINES,
        s.cleartype,
    )?);
    s.labels.insert(key, label.clone());
    Some(label)
}

fn in_rect(r: &RECT, pt: POINT) -> bool {
    pt.x >= r.left && pt.x < r.right && pt.y >= r.top && pt.y < r.bottom
}

fn item_at(s: &Shared, pt: POINT) -> Option<ItemId> {
    s.placed
        .iter()
        .find(|p| {
            in_rect(&p.cell, pt) && {
                let (icon, label) = hit_rects(s, p);
                in_rect(&icon, pt) || label.is_some_and(|l| in_rect(&l, pt))
            }
        })
        .map(|p| p.item.id)
}

fn screen_pt(hwnd: HWND, lparam: isize) -> POINT {
    window::client_to_screen(
        hwnd,
        POINT {
            x: msg::lo_i16(lparam),
            y: msg::hi_i16(lparam),
        },
    )
}

/// Makes `hwnd` the foreground window with the keyboard focus, so Delete, F2 and Enter reach
/// the selection. The press may have gone to Explorer's desktop (a rectangle drawn on it), so
/// the foreground lock is claimed the way the new-fence menu does.
fn take_keyboard(hwnd: HWND) {
    if !window::claim_foreground(hwnd) {
        tracing::debug!("free icons could not take the keyboard");
    }
    window::set_focus(hwnd);
}

fn on_message(
    shared: &Rc<RefCell<Shared>>,
    hwnd: HWND,
    m: u32,
    wparam: usize,
    lparam: isize,
) -> Option<isize> {
    if m == dragdrop::di_getdragimage_msg() {
        return Some(answer_drag_image(shared, lparam));
    }
    match m {
        WM_MOUSEACTIVATE => Some(MA_NOACTIVATE),
        msg::WM_WINDOWPOSCHANGING => {
            // Taking the keyboard (a click on an icon makes this the foreground window) must not
            // lift the layer above other windows or the fences: only the anchor orders it.
            if !crate::anchor::is_anchoring() {
                // SAFETY: lparam is the WINDOWPOS of the message being processed.
                unsafe { window::windowpos_deny_zorder_change(lparam) };
            }
            None
        }
        msg::WM_KEYDOWN => {
            const VK_A: u32 = 0x41;
            const VK_C: u32 = 0x43;
            const VK_V: u32 = 0x56;
            const VK_X: u32 = 0x58;
            let k = wparam as u32;
            let ctrl = window::key_down(msg::VK_CONTROL);
            if ctrl && k == VK_A {
                // Ctrl+A: every free icon, as on Explorer's desktop.
                let changed = {
                    let mut s = shared.try_borrow_mut().ok()?;
                    let all: Vec<ItemId> = s.placed.iter().map(|p| p.item.id).collect();
                    std::mem::replace(&mut s.selected, all.clone())
                        .into_iter()
                        .chain(all)
                        .collect::<Vec<_>>()
                };
                redraw_items(shared, &changed);
                return Some(0);
            }
            let s = shared.try_borrow().ok()?;
            if ctrl && k == VK_V {
                // Ctrl+V: the clipboard's files onto the desktop.
                s.queue.push(Command::Paste { fence: s.inbox });
                return Some(0);
            }
            let items = s.selected.clone();
            if items.is_empty() {
                return None;
            }
            if ctrl && (k == VK_C || k == VK_X) {
                // Ctrl+C / Ctrl+X: the files on the clipboard the way Explorer puts them there,
                // so Telegram, a browser or a folder can paste them.
                s.queue.push(Command::ClipboardVerb {
                    fence: s.inbox,
                    items,
                    cut: k == VK_X,
                });
                Some(0)
            } else if k == msg::VK_DELETE {
                // Explorer: Delete to the Recycle Bin, Shift+Delete for good (Windows asks).
                s.queue.push(Command::DeleteItems {
                    fence: s.inbox,
                    items,
                    permanent: window::key_down(msg::VK_SHIFT),
                });
                Some(0)
            } else if k == msg::VK_F2 {
                s.queue.push(Command::RenameItem {
                    fence: s.inbox,
                    item: items[0],
                });
                Some(0)
            } else if k == msg::VK_RETURN {
                for item in items {
                    s.queue.push(Command::LaunchItem {
                        item,
                        from: s.inbox,
                    });
                }
                Some(0)
            } else {
                None
            }
        }
        WM_NCHITTEST => {
            let pt = POINT {
                x: msg::lo_i16(lparam),
                y: msg::hi_i16(lparam),
            };
            let s = shared.try_borrow().ok()?;
            Some(if item_at(&s, pt).is_some() {
                HTCLIENT
            } else {
                HTTRANSPARENT
            })
        }
        WM_MOUSEMOVE => {
            let pt = screen_pt(hwnd, lparam);
            let (changed, drag) = {
                let mut s = shared.try_borrow_mut().ok()?;
                let hover = item_at(&s, pt);
                let old = std::mem::replace(&mut s.hover, hover);
                let changed: Vec<ItemId> = if old != hover {
                    old.into_iter().chain(hover).collect()
                } else {
                    Vec::new()
                };
                let (tx, ty) = window::drag_threshold();
                // Only with the button really held, and never inside a drag already running (its
                // loop, and the modal dialogs of a drop, pump our messages).
                let held = window::key_down_async(msg::VK_LBUTTON) && !DRAGGING.with(Cell::get);
                let drag = s
                    .press
                    .as_ref()
                    .filter(|_| held)
                    .filter(|p| (pt.x - p.start.x).abs() > tx || (pt.y - p.start.y).abs() > ty)
                    .map(|p| (p.item, p.start));
                (changed, drag)
            };
            if !changed.is_empty() {
                redraw_items(shared, &changed);
            }
            window::track_mouse_leave(hwnd);
            if let Some((item, start)) = drag {
                drag_out(shared, hwnd, item, start);
            }
            Some(0)
        }
        WM_MOUSELEAVE => {
            let old = shared
                .try_borrow_mut()
                .ok()
                .and_then(|mut s| s.hover.take());
            if let Some(old) = old {
                redraw_items(shared, &[old]);
            }
            Some(0)
        }
        WM_LBUTTONDOWN | WM_RBUTTONDOWN => {
            let pt = screen_pt(hwnd, lparam);
            let ctrl = wparam & (MK_CONTROL | MK_SHIFT) != 0;
            let changed = {
                let mut s = shared.try_borrow_mut().ok()?;
                let Some(item) = item_at(&s, pt) else {
                    return Some(0);
                };
                let before = s.selected.clone();
                let was_selected = s.selected.contains(&item);
                let mut narrow = false;
                if m == WM_LBUTTONDOWN && ctrl {
                    if was_selected {
                        s.selected.retain(|i| *i != item);
                    } else {
                        s.selected.push(item);
                    }
                } else if !was_selected {
                    s.selected = vec![item];
                } else {
                    narrow = m == WM_LBUTTONDOWN && s.selected.len() > 1;
                }
                if m == WM_LBUTTONDOWN && !(ctrl && was_selected) {
                    s.press = Some(Press {
                        item,
                        start: pt,
                        narrow_on_release: narrow,
                    });
                }
                s.queue.push(Command::ClearFenceSelection);
                let mut changed = before;
                changed.extend(s.selected.iter().copied());
                changed
            };
            // The keyboard follows the click, as on Explorer's desktop and in a fence: Delete and
            // Enter then act on the selection.
            take_keyboard(hwnd);
            if m == WM_LBUTTONDOWN {
                window::set_capture(hwnd);
            }
            redraw_items(shared, &changed);
            Some(0)
        }
        WM_LBUTTONUP => {
            let changed = {
                let mut s = shared.try_borrow_mut().ok()?;
                match s.press.take() {
                    Some(p) if p.narrow_on_release => {
                        std::mem::replace(&mut s.selected, vec![p.item])
                    }
                    _ => Vec::new(),
                }
            };
            window::release_capture();
            if !changed.is_empty() {
                redraw_items(shared, &changed);
            }
            Some(0)
        }
        WM_CAPTURECHANGED => {
            if let Ok(mut s) = shared.try_borrow_mut() {
                s.press = None;
            }
            Some(0)
        }
        WM_LBUTTONDBLCLK => {
            let pt = screen_pt(hwnd, lparam);
            let s = shared.try_borrow().ok()?;
            if let Some(item) = item_at(&s, pt) {
                let items = if s.selected.contains(&item) {
                    s.selected.clone()
                } else {
                    vec![item]
                };
                for item in items {
                    s.queue.push(Command::LaunchItem {
                        item,
                        from: s.inbox,
                    });
                }
            }
            Some(0)
        }
        WM_RBUTTONUP => {
            let pt = screen_pt(hwnd, lparam);
            let s = shared.try_borrow().ok()?;
            if item_at(&s, pt).is_some() && !s.selected.is_empty() {
                s.queue.push(Command::ItemMenu {
                    fence: s.inbox,
                    items: s.selected.clone(),
                    x: pt.x,
                    y: pt.y,
                });
            }
            Some(0)
        }
        _ => None,
    }
}

/// `DI_GETDRAGIMAGE`: the selected icons exactly as they are on screen (cells cut from the
/// layer's own bitmap), with the pointer where it holds them, as Explorer's desktop answers.
fn answer_drag_image(shared: &Rc<RefCell<Shared>>, lparam: isize) -> isize {
    let Ok(mut s) = shared.try_borrow_mut() else {
        return 0;
    };
    let pt = window::cursor_pos();
    let sel: Vec<usize> = (0..s.placed.len())
        .filter(|&i| s.selected.contains(&s.placed[i].item.id))
        .collect();
    let Some(&first) = sel.first() else {
        return 0;
    };
    let monitor = s.placed[first].monitor.clone();
    let mine: Vec<RECT> = sel
        .iter()
        .filter(|&&i| s.placed[i].monitor == monitor)
        .map(|&i| s.placed[i].cell)
        .collect();
    let mut u = mine[0];
    for r in &mine {
        u.left = u.left.min(r.left);
        u.top = u.top.min(r.top);
        u.right = u.right.max(r.right);
        u.bottom = u.bottom.max(r.bottom);
    }
    let Some(win) = s.windows.iter_mut().find(|w| w.monitor == monitor) else {
        return 0;
    };
    let origin = win.origin;
    let Some(image) = win.image.as_mut() else {
        return 0;
    };
    let (iw, ih) = image.size();
    let (w, h) = (u.right - u.left, u.bottom - u.top);
    if w <= 0 || h <= 0 {
        return 0;
    }
    let src = image.pixels_mut();
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    for r in &mine {
        for y in r.top..r.bottom {
            let sy = y - origin.y;
            if sy < 0 || sy >= ih {
                continue;
            }
            for x in r.left..r.right {
                let sx = x - origin.x;
                if sx < 0 || sx >= iw {
                    continue;
                }
                let so = ((sy * iw + sx) * 4) as usize;
                let o = (((y - u.top) * w + (x - u.left)) * 4) as usize;
                // The alpha-1 backing that takes the mouse is not part of the picture.
                if src[so + 3] > 1 {
                    bgra[o..o + 4].copy_from_slice(&src[so..so + 4]);
                }
            }
        }
    }
    let image = DragImage {
        width: w as u32,
        height: h as u32,
        offset: (pt.x - u.left, pt.y - u.top),
        bgra,
    };
    // SAFETY: `lparam` is the SHDRAGIMAGE* of the DI_GETDRAGIMAGE being handled.
    isize::from(unsafe { dragdrop::answer_drag_image(lparam, &image) })
}

/// The left button was pressed on a free icon and the pointer left the drag threshold: an
/// OLE drag of the selection with the shell's data object, as from a fence. Released on the
/// bare desktop (or a free icon that is no drop target) the icons move to the cell under the
/// pointer; on a fence they join it; on a free folder, shortcut or the Recycle Bin, or in any
/// other program, the target does what it does with files.
fn drag_out(shared: &Rc<RefCell<Shared>>, hwnd: HWND, item: ItemId, start: POINT) {
    let (items, paths, inbox, grab, queue, changed) = {
        let Ok(mut s) = shared.try_borrow_mut() else {
            return;
        };
        s.press = None;
        let mut changed = Vec::new();
        if !s.selected.contains(&item) {
            changed = std::mem::replace(&mut s.selected, vec![item]);
            changed.push(item);
        }
        let items = s.selected.clone();
        let paths: Vec<PathBuf> = s
            .placed
            .iter()
            .filter(|p| items.contains(&p.item.id))
            .map(|p| p.item.path.clone())
            .collect();
        let grab = s
            .placed
            .iter()
            .find(|p| p.item.id == item)
            .map(|p| (start.x - p.cell.left, start.y - p.cell.top))
            .unwrap_or((0, 0));
        (items, paths, s.inbox, grab, s.queue.clone(), changed)
    };
    if !changed.is_empty() {
        redraw_items(shared, &changed);
    }
    window::release_capture();
    let paths = pecofence_platform::shell::paths_for_shell(paths);
    let refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let Ok(obj) = dragdrop::data_object_for_paths(&refs) else {
        tracing::warn!("shell data object for a free icon drag failed");
        return;
    };
    crate::fence_window::set_internal_drag(inbox, items.clone(), paths.clone());
    crate::peek::set_drag_click_through(true);
    DRAGGING.with(|d| d.set(true));
    let outcome = dragdrop::do_drag_drop(
        hwnd,
        &obj,
        dragdrop::ALL_EFFECTS,
        Some(Box::new(skip_drop_at_pointer)),
    );
    DRAGGING.with(|d| d.set(false));
    crate::fence_window::clear_internal_drag();
    let (_, class, on_desktop) = pointer_target();
    crate::peek::set_drag_click_through(false);
    let into_item = take_item_drop();
    tracing::info!(
        outcome = ?(outcome.dropped, outcome.effect),
        on_desktop,
        into_item,
        %class,
        "free icon drag finished"
    );
    if !outcome.dropped {
        return;
    }
    if into_item {
        // A folder / shortcut / the Recycle Bin took the files: the desktop changes by itself.
        queue.push(Command::DragOutFinished);
    } else if on_desktop && matches!(outcome.effect, DropEffect::None | DropEffect::Move) {
        let at = window::cursor_pos();
        queue.push(Command::PlaceOnDesktop {
            items,
            at: (at.x, at.y),
            grab: Some((item, grab.0, grab.1)),
        });
    } else if matches!(outcome.effect, DropEffect::Move | DropEffect::Copy) && class != FENCE_CLASS
    {
        queue.push(Command::DragOutFinished);
    }
}

// ---- drops onto free icons ---------------------------------------------------------------------

/// The drop target of a layer window. Over a free item that takes the data (its own shell drop
/// target says so: a folder, a shortcut, the Recycle Bin, a program) the drag goes to that
/// target and the item is highlighted; elsewhere on the layer it is the desktop: our own icons
/// move (the drag source places them), files from other programs land on the desktop through
/// the desktop folder's drop target, as on Explorer's desktop.
struct LayerDrop {
    shared: Weak<RefCell<Shared>>,
    hwnd: HWND,
    helper: Option<DropTargetHelper>,
    data: Option<IDataObject>,
    /// Items of our own drag (they are no targets for themselves).
    internal: Vec<ItemId>,
    /// The item the drag is over and its drop target, when it takes the data; the folder it
    /// stands for (itself, the folder a shortcut points at, the Recycle Bin), if any.
    target: Option<(ItemId, ItemDropTarget)>,
    target_folder: Option<PathBuf>,
    /// The effect the target reported last.
    effect: DropEffect,
    key_state: u32,
    allowed: u32,
}

impl LayerDrop {
    fn set_highlight(&self, item: Option<ItemId>) {
        let Some(shared) = self.shared.upgrade() else {
            return;
        };
        let old = {
            let Ok(mut s) = shared.try_borrow_mut() else {
                return;
            };
            std::mem::replace(&mut s.drop_hover, item)
        };
        if old != item {
            let changed: Vec<ItemId> = old.into_iter().chain(item).collect();
            redraw_items(&shared, &changed);
        }
    }

    /// The free item under `pt` that takes the data (entering its drop target when the pointer
    /// moved onto another item), and the effect it would perform.
    fn retarget(&mut self, pt: DragPoint) -> Option<DropEffect> {
        let shared = self.shared.upgrade()?;
        let hit = {
            let s = shared.try_borrow().ok()?;
            item_at(&s, POINT { x: pt.x, y: pt.y }).and_then(|id| {
                s.placed
                    .iter()
                    .find(|p| p.item.id == id)
                    .map(|p| (id, p.item.path.clone()))
            })
        };
        let hit = hit.filter(|(id, _)| !self.internal.contains(id));
        if self.target.as_ref().map(|(id, _)| *id) == hit.as_ref().map(|(id, _)| *id) {
            return self
                .target
                .as_ref()
                .map(|(_, t)| t.over(self.key_state, pt, self.allowed));
        }
        if let Some((_, mut old)) = self.target.take() {
            old.leave();
        }
        let (id, path) = hit?;
        let data = self.data.clone()?;
        // A shortcut to a folder: the folder it points at takes the drop (Explorer's result).
        let path = crate::fence_window::folder_link_target(&path, 0).unwrap_or(path);
        let mut target = ItemDropTarget::for_path(&path)?;
        let effect = target.enter(&data, self.key_state, pt, self.allowed);
        if effect == DropEffect::None {
            target.leave();
            return None;
        }
        self.target_folder = (path.is_dir()
            || pecofence_platform::shell::is_recycle_bin_path(&path))
        .then_some(path);
        self.target = Some((id, target));
        Some(effect)
    }

    /// The effect over the bare part of the layer (or an item that takes nothing): a move of
    /// our own icons; for files from elsewhere what the desktop folder would do with them.
    fn desktop_effect(&self, key_state: u32) -> DropEffect {
        if !self.internal.is_empty() {
            return dragdrop::resolve(DropEffect::Move, self.allowed);
        }
        let same =
            dragdrop::same_volume(None, pecofence_platform::shell::user_desktop().as_deref());
        dragdrop::resolve(dragdrop::modifier_effect(key_state, same), self.allowed)
    }

    fn update(&mut self, pt: DragPoint) -> DropEffect {
        match self.retarget(pt) {
            Some(effect) => {
                let id = self.target.as_ref().map(|(id, _)| *id);
                self.set_highlight(id);
                self.effect = effect;
                effect
            }
            None => {
                self.set_highlight(None);
                self.target_folder = None;
                self.effect = self.desktop_effect(self.key_state);
                self.effect
            }
        }
    }
}

impl DropHandler for LayerDrop {
    fn drag_enter(
        &mut self,
        data: &IDataObject,
        key_state: u32,
        pt: DragPoint,
        allowed: u32,
    ) -> DropEffect {
        self.data = Some(data.clone());
        self.internal = crate::fence_window::internal_drag_items();
        self.key_state = key_state;
        self.allowed = allowed;
        let effect = self.update(pt);
        if let Some(h) = &self.helper {
            h.drag_enter(self.hwnd, data, pt, effect);
        }
        effect
    }

    fn drag_over(&mut self, key_state: u32, pt: DragPoint, allowed: u32) -> DropEffect {
        self.key_state = key_state;
        self.allowed = allowed;
        let effect = self.update(pt);
        if let Some(h) = &self.helper {
            h.drag_over(pt, effect);
        }
        effect
    }

    fn drag_leave(&mut self) {
        if let Some((_, mut t)) = self.target.take() {
            t.leave();
        }
        self.set_highlight(None);
        self.data = None;
        if let Some(h) = &self.helper {
            h.drag_leave();
        }
    }

    fn drop(
        &mut self,
        data: &IDataObject,
        key_state: u32,
        pt: DragPoint,
        allowed: u32,
    ) -> DropEffect {
        self.key_state = key_state;
        self.allowed = allowed;
        let _ = self.update(pt);
        self.set_highlight(None);
        if let Some(h) = &self.helper {
            h.drop(data, pt, DropEffect::Copy);
        }
        self.data = None;
        if let Some((id, mut target)) = self.target.take() {
            // Our own drag: its source must not take the drop for a move of icons. (A drag
            // from another program never reads the flag, so it is not set for one.)
            if !self.internal.is_empty() {
                ITEM_DROP.with(|d| d.set(true));
            }
            // Our own icons into a folder (or the Recycle Bin): the shell's file operation on a
            // worker thread, as for a folder inside a fence, so its progress, conflict and
            // error dialogs never hold this window loop. The mode is what the folder's own drop
            // target chose for the modifiers held.
            if let Some(folder) = self.target_folder.take()
                && !self.internal.is_empty()
            {
                target.leave();
                let mode = match self.effect {
                    DropEffect::Copy => Some(TransferMode::Copy),
                    DropEffect::Link => Some(TransferMode::Link),
                    DropEffect::Move => Some(TransferMode::Move),
                    DropEffect::None => None,
                };
                let paths = crate::fence_window::filter_folder_paths(
                    crate::fence_window::internal_drag_paths(),
                    &folder,
                );
                if let (Some(mode), Some(shared), false) =
                    (mode, self.shared.upgrade(), paths.is_empty())
                    && let Ok(s) = shared.try_borrow()
                {
                    tracing::info!(%id, ?mode, files = paths.len(), "free icons dropped into a folder");
                    s.queue.push(Command::DropIntoFolder {
                        paths,
                        folder,
                        fence: s.inbox,
                        mode,
                    });
                }
                return self.effect;
            }
            tracing::info!(%id, "drop handed to a free item's shell drop target");
            return target
                .drop(data, key_state, pt, allowed)
                .map_err(|e| tracing::warn!(error = %e, "drop onto a free item failed"))
                .unwrap_or(DropEffect::None);
        }
        if !self.internal.is_empty() {
            // Our own icons over the desktop: the drag source places them.
            return DropEffect::None;
        }
        // Files from elsewhere: onto the desktop, as Explorer's desktop takes them.
        let Some(desk) = pecofence_platform::shell::user_desktop() else {
            return DropEffect::None;
        };
        let Some(mut target) = ItemDropTarget::for_path(&desk) else {
            return DropEffect::None;
        };
        if target.enter(data, key_state, pt, allowed) == DropEffect::None {
            return DropEffect::None;
        }
        let effect = target
            .drop(data, key_state, pt, allowed)
            .map_err(|e| tracing::warn!(error = %e, "drop onto the desktop failed"))
            .unwrap_or(DropEffect::None);
        if effect != DropEffect::None
            && let Some(shared) = self.shared.upgrade()
            && let Ok(s) = shared.try_borrow()
        {
            let names: Vec<String> = dragdrop::hdrop_paths(data)
                .iter()
                .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                .collect();
            s.queue.push(Command::PlaceNewOnDesktop {
                names,
                at: (pt.x, pt.y),
            });
        }
        effect
    }
}

// ---- drawing -----------------------------------------------------------------------------------

/// Redraws every window from scratch (layout, visibility or bitmap size changed).
fn redraw_all(shared: &Rc<RefCell<Shared>>) {
    let Ok(mut s) = shared.try_borrow_mut() else {
        return;
    };
    let started = std::time::Instant::now();
    let icons = s.ctx.icons.clone();
    let mut icons = icons.borrow_mut();
    let mut pending = false;
    for i in 0..s.windows.len() {
        if !s.visible {
            s.windows[i].window.hide();
            continue;
        }
        let monitor = s.windows[i].monitor.clone();
        let mine: Vec<usize> = (0..s.placed.len())
            .filter(|&k| s.placed[k].monitor == monitor)
            .collect();
        let Some(&first) = mine.first() else {
            s.windows[i].window.hide();
            continue;
        };
        let mut bounds = s.placed[first].cell;
        for &k in &mine {
            let c = s.placed[k].cell;
            bounds.left = bounds.left.min(c.left);
            bounds.top = bounds.top.min(c.top);
            bounds.right = bounds.right.max(c.right);
            bounds.bottom = bounds.bottom.max(c.bottom);
        }
        let (w, h) = (bounds.right - bounds.left, bounds.bottom - bounds.top);
        let Ok(mut image) = LayeredImage::blank(w, h) else {
            continue;
        };
        let origin = POINT {
            x: bounds.left,
            y: bounds.top,
        };
        for &k in &mine {
            pending |= draw_cell(&mut s, &mut icons, k, image.pixels_mut(), w, h, origin);
        }
        let hwnd = s.windows[i].window.hwnd();
        if let Err(e) = image.present_dirty(hwnd, origin.x, origin.y, None) {
            tracing::warn!(error = %e, "desktop layer present failed");
        }
        s.windows[i].origin = origin;
        s.windows[i].image = Some(image);
        if !s.windows[i].window.is_visible() {
            s.windows[i].window.show_no_activate();
        }
    }
    s.pending_icons = pending;
    tracing::debug!(
        ms = started.elapsed().as_secs_f32() * 1000.0,
        icons = s.placed.len(),
        "desktop layer drawn"
    );
}

/// Redraws only the cells of `items` and presents just that part of each window.
fn redraw_items(shared: &Rc<RefCell<Shared>>, items: &[ItemId]) {
    if items.is_empty() {
        return;
    }
    let Ok(mut s) = shared.try_borrow_mut() else {
        return;
    };
    if !s.visible {
        return;
    }
    let icons = s.ctx.icons.clone();
    let mut icons = icons.borrow_mut();
    let mut pending = false;
    for i in 0..s.windows.len() {
        let monitor = s.windows[i].monitor.clone();
        let origin = s.windows[i].origin;
        let hwnd = s.windows[i].window.hwnd();
        let cells: Vec<usize> = (0..s.placed.len())
            .filter(|&k| s.placed[k].monitor == monitor && items.contains(&s.placed[k].item.id))
            .collect();
        if cells.is_empty() {
            continue;
        }
        let Some(mut image) = s.windows[i].image.take() else {
            continue;
        };
        let (w, h) = image.size();
        let mut dirty: Option<RECT> = None;
        for &k in &cells {
            let c = s.placed[k].cell;
            let r = RECT {
                left: (c.left - origin.x).max(0),
                top: (c.top - origin.y).max(0),
                right: (c.right - origin.x).min(w),
                bottom: (c.bottom - origin.y).min(h),
            };
            let px = image.pixels_mut();
            for y in r.top..r.bottom {
                let o = ((y * w + r.left) * 4) as usize;
                px[o..o + ((r.right - r.left) * 4) as usize].fill(0);
            }
            pending |= draw_cell(&mut s, &mut icons, k, image.pixels_mut(), w, h, origin);
            dirty = Some(match dirty {
                None => r,
                Some(d) => RECT {
                    left: d.left.min(r.left),
                    top: d.top.min(r.top),
                    right: d.right.max(r.right),
                    bottom: d.bottom.max(r.bottom),
                },
            });
        }
        if let Err(e) = image.present_dirty(hwnd, origin.x, origin.y, dirty) {
            tracing::warn!(error = %e, "desktop layer present failed");
        }
        s.windows[i].image = Some(image);
    }
    s.pending_icons = pending;
}

/// Premultiplied "over" of a colour with `alpha` (0..=1) onto one BGRA pixel.
fn blend(px: &mut [u8], b: u8, g: u8, r: u8, alpha: f32) {
    let a = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    px[0] = (b as f32 * a + px[0] as f32 * inv).round() as u8;
    px[1] = (g as f32 * a + px[1] as f32 * inv).round() as u8;
    px[2] = (r as f32 * a + px[2] as f32 * inv).round() as u8;
    px[3] = (255.0 * a + px[3] as f32 * inv).round() as u8;
}

/// Draws free icon `k` into `buf` (a `w` x `h` bitmap whose top-left is screen point `origin`).
/// Returns true when its picture is still being extracted (the last one is drawn meanwhile).
fn draw_cell(
    s: &mut Shared,
    icons: &mut IconCache,
    k: usize,
    buf: &mut [u8],
    w: i32,
    h: i32,
    origin: POINT,
) -> bool {
    let label = ensure_label(s, k);
    let p = &s.placed[k];
    let id = p.item.id;
    let sc = p.dpi as f32 / 96.0;
    let cell = p.cell;
    let (icon_box, label_box) = hit_rects(s, p);
    let selected = s.selected.contains(&id) || s.drop_hover == Some(id);
    let hovered = s.hover == Some(id);
    let put = |x: i32, y: i32| -> Option<usize> {
        let (bx, by) = (x - origin.x, y - origin.y);
        (bx >= 0
            && by >= 0
            && bx < w
            && by < h
            && x >= cell.left
            && x < cell.right
            && y >= cell.top
            && y < cell.bottom)
            .then(|| ((by * w + bx) * 4) as usize)
    };
    // The item's area (icon box, label lines): backed at alpha 1 so it takes the mouse, then
    // Windows 11's hover / selection fill over the icon box and label together.
    let area = RECT {
        left: icon_box.left,
        top: icon_box.top,
        right: icon_box.right,
        bottom: label_box.map_or(icon_box.bottom, |l| l.bottom),
    };
    let (fill, border) = match (selected, hovered) {
        (true, true) => (0.42, 0.62),
        (true, false) => (0.32, 0.55),
        (false, true) => (0.18, 0.0),
        (false, false) => (0.0, 0.0),
    };
    let taken = |x: i32, y: i32| {
        (x >= icon_box.left && x < icon_box.right && y >= icon_box.top && y < icon_box.bottom)
            || label_box.is_some_and(|l| x >= l.left && x < l.right && y >= l.top && y < l.bottom)
    };
    for y in area.top..area.bottom {
        for x in area.left..area.right {
            let Some(o) = put(x, y) else { continue };
            let px = &mut buf[o..o + 4];
            if fill > 0.0 {
                blend(px, 255, 232, 204, fill);
                let edge =
                    y == area.top || y == area.bottom - 1 || x == area.left || x == area.right - 1;
                if border > 0.0 && edge {
                    blend(px, 255, 209, 153, border);
                }
            } else if taken(x, y) {
                px[3] = px[3].max(1);
            }
        }
    }
    // The picture: centred, 2 px below the cell's top. The last one drawn stands in while the
    // cache extracts it again.
    let icon_px = (s.icon_dip as f32 * sc).round() as u32;
    let key = icons.draw_key(&p.item.icon, icon_px);
    let mut pending = false;
    let img = match icons.get(&p.item.icon, &p.item.path, icon_px, p.dpi, false) {
        Lookup::Ready(Some(img)) => {
            s.images.insert(id, (key, img.clone()));
            Some(img)
        }
        Lookup::Ready(None) => None,
        Lookup::Pending => {
            pending = true;
            s.images
                .get(&id)
                .filter(|(k, _)| *k == key)
                .map(|(_, img)| img.clone())
        }
    };
    let p = &s.placed[k];
    let cw = cell.right - cell.left;
    if let Some(img) = img {
        let ix = cell.left + (cw - img.width as i32) / 2;
        let iy = cell.top
            + (DESK_ICON_TOP_DIP * sc).round() as i32
            + (icon_px as i32 - img.height as i32) / 2;
        for y in 0..img.height as i32 {
            for x in 0..img.width as i32 {
                let so = ((y * img.width as i32 + x) * 4) as usize;
                let sa = img.bgra[so + 3] as f32 / 255.0;
                if sa <= 0.0 {
                    continue;
                }
                let Some(o) = put(ix + x, iy + y) else {
                    continue;
                };
                let inv = 1.0 - sa;
                for c in 0..3 {
                    buf[o + c] = (img.bgra[so + c] as f32 + buf[o + c] as f32 * inv).round() as u8;
                }
                buf[o + 3] = (img.bgra[so + 3] as f32 + buf[o + 3] as f32 * inv)
                    .round()
                    .max(1.0) as u8;
            }
        }
    }
    let _ = p;
    // The label: a dark shadow one pixel down and right, softened, then the white text with its
    // own (ClearType) coverage per colour channel over it, as Explorer draws desktop labels.
    if let Some(l) = label {
        let lx = cell.left;
        let ly = cell.top + (label_top_dip(s) * sc).round() as i32;
        let cov = |x: i32, y: i32| -> f32 {
            if x < 0 || y < 0 || x >= l.width || y >= l.height {
                0.0
            } else {
                let c = l.coverage[(y * l.width + x) as usize];
                c[0].max(c[1]).max(c[2]) as f32 / 255.0
            }
        };
        for y in -1..l.height + 2 {
            for x in -1..l.width + 2 {
                let Some(o) = put(lx + x, ly + y) else {
                    continue;
                };
                let mut sh = 0.0;
                for (ox, oy, wgt) in [
                    (-1, -1, 0.62),
                    (0, -1, 0.3),
                    (-1, 0, 0.3),
                    (-2, -1, 0.12),
                    (-1, -2, 0.12),
                    (0, 0, 0.18),
                ] {
                    sh += cov(x + ox, y + oy) * wgt;
                }
                let shadow = sh.min(0.86);
                if shadow > 0.0 {
                    blend(&mut buf[o..o + 4], 0, 0, 0, shadow);
                }
                if x < 0 || y < 0 || x >= l.width || y >= l.height {
                    continue;
                }
                let c = l.coverage[(y * l.width + x) as usize];
                let a = c[0].max(c[1]).max(c[2]) as f32 / 255.0;
                if a <= 0.0 {
                    continue;
                }
                // White text: each channel by its own coverage (B, G, R in the bitmap).
                let px = &mut buf[o..o + 4];
                for (ch, cv) in [(0usize, c[2]), (1, c[1]), (2, c[0])] {
                    let t = cv as f32 / 255.0;
                    px[ch] = (255.0 * t + px[ch] as f32 * (1.0 - t)).round() as u8;
                }
                px[3] = (255.0 * a + px[3] as f32 * (1.0 - a)).round() as u8;
            }
        }
    }
    pending
}
