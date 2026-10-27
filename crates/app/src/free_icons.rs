//! Free desktop icons (MistFences). An item in no fence sits on the desktop itself, like an
//! icon on the Windows desktop: the inbox fence stays in the data model as their category but
//! has no window, and each member's `ItemRef::desk` says where it sits. `desktop_layer` draws
//! them; this file keeps their places: new items get the first free cell, dropped ones the cell
//! under the drop, and nothing ever moves a file.

use crate::state::AppState;
use pecofence_core::desk::DipRect;
use pecofence_core::geometry::WorkArea;
use pecofence_core::{AppearanceOverride, AssignedBy, DeskPos, FenceId, FenceKind, ItemId};

/// Height of a fence's title row in DIPs (the themes' `title_height`), where its items start.
const FENCE_TITLE_DIP: f32 = 36.0;
/// The icon's top inside a free icon's cell, as Explorer draws it (2 DIPs below the cell's top,
/// centred in its width); `desktop_layer` draws it there.
pub const DESK_ICON_TOP_DIP: f32 = 2.0;

/// A free icon where it is shown now.
#[derive(Clone, Debug, PartialEq)]
pub struct FreeIcon {
    pub item: ItemId,
    /// The work area it is on (a connected monitor).
    pub monitor: String,
    /// Cell top-left in DIPs within that work area.
    pub x: f32,
    pub y: f32,
}

/// An imported cluster of loose icons from earlier MistFences versions: a transparent fence
/// with its title on hover and a fixed column count (`fences_import`).
fn is_loose_cluster(f: &pecofence_core::Fence) -> bool {
    f.kind == FenceKind::Virtual
        && f.view.columns.is_some()
        && matches!(
            f.appearance,
            Some(AppearanceOverride {
                opacity: Some(o),
                title_on_hover: Some(true),
                ..
            }) if o <= 0.0
        )
}

impl AppState {
    fn work_area(&self, monitor: &str) -> Option<&WorkArea> {
        self.work_areas.iter().find(|w| w.device_path == monitor)
    }

    /// The primary monitor's work area (free icons without a place start there).
    fn primary_area(&self) -> Option<&WorkArea> {
        self.work_areas
            .iter()
            .find(|w| w.mon_left == 0 && w.mon_top == 0)
            .or(self.work_areas.first())
    }

    /// The fences shown on `work` as DIP rectangles within it (a rolled fence as its title
    /// strip): free icons are never placed under them.
    fn fence_rects_on(&self, work: &WorkArea) -> Vec<DipRect> {
        let s = work.scale();
        self.host_fences()
            .iter()
            .filter(|f| f.kind != FenceKind::Inbox)
            .filter_map(|f| {
                let px = self.fence_px_rect(f);
                let (cx, cy) = px.center();
                let on = cx >= work.left && cx < work.right && cy >= work.top && cy < work.bottom;
                on.then(|| DipRect {
                    x: (px.left - work.left) as f32 / s,
                    y: (px.top - work.top) as f32 / s,
                    w: px.width() as f32 / s,
                    h: if f.rolled_up {
                        FENCE_TITLE_DIP
                    } else {
                        px.height() as f32 / s
                    },
                })
            })
            .collect()
    }

    /// Whether `item` is shown on the desktop: it exists and its file has not gone missing. A
    /// missing one keeps its saved place for its return but takes no cell meanwhile.
    fn on_desk(&self, item: ItemId) -> bool {
        self.item(item)
            .is_some_and(|it| it.orphaned_since.is_none())
    }

    /// Positions of the other free icons on `monitor` (cells taken), leaving out `except`.
    fn taken_on(&self, monitor: &str, except: &[ItemId]) -> Vec<(f32, f32)> {
        let Some(inbox) = self.inbox_id().and_then(|id| self.fence(id)) else {
            return Vec::new();
        };
        inbox
            .items
            .iter()
            .filter(|r| !except.contains(&r.item_id) && self.on_desk(r.item_id))
            .filter_map(|r| r.desk.as_ref())
            .filter(|d| d.monitor == monitor)
            .map(|d| (d.x, d.y))
            .collect()
    }

    fn set_desk(&mut self, item: ItemId, pos: Option<DeskPos>) {
        let Some(inbox) = self.inbox_id() else {
            return;
        };
        if let Some(f) = self.fence_mut(inbox)
            && let Some(r) = f.items.iter_mut().find(|r| r.item_id == item)
            && r.desk != pos
        {
            r.desk = pos;
            self.mark_dirty();
        }
    }

    /// Gives every free icon without a place (a new desktop item, one moved out of a fence by
    /// a menu) the first free cell in Explorer's order on the primary monitor. True when any
    /// got one.
    pub fn place_free_items(&mut self) -> bool {
        let grid = self.desk_grid;
        let Some(inbox) = self.inbox_id().and_then(|id| self.fence(id)) else {
            return false;
        };
        let mut unplaced: Vec<(Option<u32>, ItemId)> = inbox
            .items
            .iter()
            .filter(|r| r.desk.is_none())
            .filter(|r| {
                self.item(r.item_id)
                    .is_some_and(|it| it.orphaned_since.is_none())
            })
            .map(|r| (r.manual_index, r.item_id))
            .collect();
        if unplaced.is_empty() {
            return false;
        }
        unplaced.sort_by_key(|(i, _)| i.unwrap_or(u32::MAX));
        let Some(work) = self.primary_area().cloned() else {
            return false;
        };
        let blocked = self.fence_rects_on(&work);
        let mut taken = self.taken_on(&work.device_path, &[]);
        let (ww, wh) = (work.width_dip(), work.height_dip());
        let mut placed = false;
        for (_, item) in unplaced {
            // A full desktop stacks the rest on the last cell rather than losing them.
            let (x, y) = grid
                .free_cell(ww, wh, &taken, &blocked, None)
                .unwrap_or_else(|| grid.snap(ww, wh, ww, wh));
            taken.push((x, y));
            self.set_desk(
                item,
                Some(DeskPos {
                    monitor: work.device_path.clone(),
                    x,
                    y,
                }),
            );
            self.make_room_for(&work, (x, y), &taken, &blocked);
            placed = true;
        }
        placed
    }

    /// Puts `wanted` items on the desktop at (about) the given cells: each becomes a free icon
    /// (leaving its fence, if it was in one) and takes the grid cell nearest to its wish that
    /// no other icon or fence has. Files stay where they are. Returns how many were placed.
    pub fn place_on_desktop(&mut self, wanted: &[(ItemId, DeskPos)]) -> usize {
        let grid = self.desk_grid;
        let snap = self.config.settings.desk_snap_to_grid;
        let Some(inbox) = self.inbox_id() else {
            return 0;
        };
        let layout = self.layout;
        let moving: Vec<ItemId> = wanted.iter().map(|(id, _)| *id).collect();
        let mut done: Vec<(String, (f32, f32))> = Vec::new();
        let mut n = 0;
        for (item, want) in wanted {
            if self.item(*item).is_none() || self.is_portal_item(*item) {
                continue;
            }
            let Some(work) = self
                .work_area(&want.monitor)
                .or_else(|| self.primary_area())
                .cloned()
            else {
                continue;
            };
            if self.config.fence_of_item(layout, *item) != Some(inbox) {
                self.config.assign(layout, *item, inbox, AssignedBy::User);
            }
            let (ww, wh) = (work.width_dip(), work.height_dip());
            let mut taken = self.taken_on(&work.device_path, &moving);
            taken.extend(
                done.iter()
                    .filter(|(m, _)| *m == work.device_path)
                    .map(|(_, c)| *c),
            );
            let blocked = self.fence_rects_on(&work);
            let cell = if snap {
                // Explorer with "Align icons to grid": the nearest free cell.
                let (sx, sy) = grid.snap(want.x, want.y, ww, wh);
                if grid.is_taken(sx, sy, &taken, &blocked) {
                    grid.free_cell(ww, wh, &taken, &blocked, Some((sx, sy)))
                        .unwrap_or((sx, sy))
                } else {
                    (sx, sy)
                }
            } else {
                // Off the grid: exactly where it was dropped, kept on the screen.
                grid.clamp_into(want.x.round(), want.y.round(), ww, wh)
            };
            done.push((work.device_path.clone(), cell));
            self.set_desk(
                *item,
                Some(DeskPos {
                    monitor: work.device_path.clone(),
                    x: cell.0,
                    y: cell.1,
                }),
            );
            taken.push(cell);
            self.make_room_for(&work, cell, &taken, &blocked);
            n += 1;
        }
        if n > 0 {
            self.mark_dirty();
        }
        n
    }

    /// `cell` on `work` was just given to an icon: an icon whose file is missing and that
    /// saved this cell moves its saved place to the nearest free cell, so it comes back beside
    /// the new one instead of under it.
    fn make_room_for(
        &mut self,
        work: &WorkArea,
        cell: (f32, f32),
        taken: &[(f32, f32)],
        blocked: &[DipRect],
    ) {
        let grid = self.desk_grid;
        let Some(inbox) = self.inbox_id().and_then(|id| self.fence(id)) else {
            return;
        };
        let missing: Vec<ItemId> = inbox
            .items
            .iter()
            .filter(|r| !self.on_desk(r.item_id))
            .filter(|r| {
                r.desk.as_ref().is_some_and(|d| {
                    d.monitor == work.device_path && grid.is_taken(d.x, d.y, &[cell], &[])
                })
            })
            .map(|r| r.item_id)
            .collect();
        let (ww, wh) = (work.width_dip(), work.height_dip());
        let mut taken = taken.to_vec();
        for item in missing {
            if let Some(free) = grid.free_cell(ww, wh, &taken, blocked, Some(cell)) {
                taken.push(free);
                self.set_desk(
                    item,
                    Some(DeskPos {
                        monitor: work.device_path.clone(),
                        x: free.0,
                        y: free.1,
                    }),
                );
            }
        }
    }

    /// The free icons as they are shown now: saved places on connected monitors (kept inside
    /// a work area that got smaller), and icons whose monitor is gone on free cells of the
    /// primary one for the time being. Their saved places stay for the monitor's return.
    pub fn free_icons(&self) -> Vec<FreeIcon> {
        let grid = self.desk_grid;
        let Some(inbox) = self.inbox_id().and_then(|id| self.fence(id)) else {
            return Vec::new();
        };
        let mut out: Vec<FreeIcon> = Vec::new();
        let mut homeless = Vec::new();
        for r in &inbox.items {
            if !self.on_desk(r.item_id) {
                continue;
            }
            match r
                .desk
                .as_ref()
                .and_then(|d| Some((d, self.work_area(&d.monitor)?)))
            {
                Some((d, work)) => {
                    let (ww, wh) = (work.width_dip(), work.height_dip());
                    let (mut x, mut y) = grid.clamp_into(d.x, d.y, ww, wh);
                    // Its cell was given to another icon while its file was missing: it shows
                    // on the nearest free cell instead of on top of that one.
                    let taken: Vec<(f32, f32)> = out
                        .iter()
                        .filter(|f| f.monitor == d.monitor)
                        .map(|f| (f.x, f.y))
                        .collect();
                    if grid.is_taken(x, y, &taken, &[]) {
                        let blocked = self.fence_rects_on(work);
                        if let Some(cell) = grid.free_cell(ww, wh, &taken, &blocked, Some((x, y))) {
                            (x, y) = cell;
                        }
                    }
                    out.push(FreeIcon {
                        item: r.item_id,
                        monitor: d.monitor.clone(),
                        x,
                        y,
                    });
                }
                None => homeless.push((r.manual_index, r.item_id)),
            }
        }
        if !homeless.is_empty()
            && let Some(work) = self.primary_area()
        {
            homeless.sort_by_key(|(i, _)| i.unwrap_or(u32::MAX));
            let blocked = self.fence_rects_on(work);
            let mut taken: Vec<(f32, f32)> = out
                .iter()
                .filter(|f| f.monitor == work.device_path)
                .map(|f| (f.x, f.y))
                .collect();
            let (ww, wh) = (work.width_dip(), work.height_dip());
            for (_, item) in homeless {
                let (x, y) = grid
                    .free_cell(ww, wh, &taken, &blocked, None)
                    .unwrap_or_else(|| grid.snap(ww, wh, ww, wh));
                taken.push((x, y));
                out.push(FreeIcon {
                    item,
                    monitor: work.device_path.clone(),
                    x,
                    y,
                });
            }
        }
        out
    }

    /// How many free icons sit off the desktop grid (saved before the grid was Explorer's, or
    /// dropped while snapping was off).
    pub fn free_icons_off_grid(&self) -> usize {
        let Some(inbox) = self.inbox_id().and_then(|id| self.fence(id)) else {
            return 0;
        };
        inbox
            .items
            .iter()
            .filter(|r| self.on_desk(r.item_id))
            .filter_map(|r| r.desk.as_ref())
            .filter(|d| self.work_area(&d.monitor).is_some())
            .filter(|d| !self.desk_grid.is_on_grid(d.x, d.y))
            .count()
    }

    /// Puts every free icon on the desktop grid, monitor by monitor: each takes the free cell
    /// nearest to where it is, in reading order, so rows stay rows and no two share a cell.
    /// Icons of monitors that are not connected keep their places. Returns how many moved.
    /// Only `ItemRef::desk` changes; files are not touched.
    pub fn align_free_icons(&mut self) -> usize {
        let Some(inbox) = self.inbox_id() else {
            return 0;
        };
        let grid = self.desk_grid;
        let mut moves: Vec<(ItemId, DeskPos)> = Vec::new();
        for work in self.work_areas.clone() {
            let Some(f) = self.fence(inbox) else {
                return 0;
            };
            let mine: Vec<(ItemId, f32, f32)> = f
                .items
                .iter()
                .filter(|r| self.on_desk(r.item_id))
                .filter_map(|r| r.desk.as_ref().map(|d| (r.item_id, d)))
                .filter(|(_, d)| d.monitor == work.device_path)
                .map(|(id, d)| (id, d.x, d.y))
                .collect();
            let wanted: Vec<(f32, f32)> = mine.iter().map(|(_, x, y)| (*x, *y)).collect();
            let cells = grid.align(&wanted, work.width_dip(), work.height_dip());
            for ((id, x, y), (cx, cy)) in mine.iter().zip(cells) {
                if (cx - x).abs() > 0.05 || (cy - y).abs() > 0.05 {
                    moves.push((
                        *id,
                        DeskPos {
                            monitor: work.device_path.clone(),
                            x: cx,
                            y: cy,
                        },
                    ));
                }
            }
        }
        let n = moves.len();
        for (id, pos) in moves {
            self.set_desk(id, Some(pos));
        }
        n
    }

    /// Where `fence` (about to be deleted) sits, as wishes for its items' cells: a block of
    /// columns from its top-left corner, so its icons stay where the fence was.
    pub fn desk_wishes_for(&self, fence: FenceId, items: &[ItemId]) -> Vec<(ItemId, DeskPos)> {
        let Some(f) = self.fence(fence) else {
            return Vec::new();
        };
        let px = self.fence_px_rect(f);
        let Some(work) = pecofence_core::geometry::resolve_work_area(&f.geometry, &self.work_areas)
        else {
            return Vec::new();
        };
        let s = work.scale();
        let (cw, ch) = (self.desk_grid.cell_w, self.desk_grid.cell_h);
        let x0 = (px.left - work.left) as f32 / s;
        let y0 = (px.top - work.top) as f32 / s;
        let rows = ((px.height() as f32 / s / ch).floor() as usize).max(1);
        items
            .iter()
            .enumerate()
            .map(|(i, id)| {
                (
                    *id,
                    DeskPos {
                        monitor: work.device_path.clone(),
                        x: x0 + (i / rows) as f32 * cw,
                        y: y0 + (i % rows) as f32 * ch,
                    },
                )
            })
            .collect()
    }

    /// Deletes a fence; its icons become free icons where the fence was (files stay put).
    pub fn delete_fence_to_desktop(&mut self, fence: FenceId) -> bool {
        let items: Vec<ItemId> = self
            .fence(fence)
            .filter(|f| f.kind != FenceKind::FolderPortal)
            .map(|f| self.items_of(f).iter().map(|it| it.id).collect())
            .unwrap_or_default();
        let wishes = self.desk_wishes_for(fence, &items);
        if !self.delete_fence(fence) {
            return false;
        }
        self.place_on_desktop(&wishes);
        true
    }

    /// Converts what earlier versions showed as fences into free icons, keeping where each
    /// icon was on screen: the inbox fence's members, and the transparent clusters an import
    /// from Fences made for loose icons (those fences go). The inbox leaves any tab group (it
    /// has no window any more). True when anything changed.
    pub fn migrate_free_desktop(&mut self) -> bool {
        let Some(inbox) = self.inbox_id() else {
            return false;
        };
        let mut changed = false;
        if self.fence(inbox).is_some_and(|f| f.tab_host.is_some()) || self.tabs_of(inbox).len() > 1
        {
            self.detach_tab(inbox);
            for tab in self.tabs_of(inbox) {
                if tab != inbox {
                    self.detach_tab(tab);
                }
            }
            changed = true;
        }
        let sources: Vec<FenceId> = std::iter::once(inbox)
            .chain(
                self.fences()
                    .iter()
                    .filter(|f| is_loose_cluster(f))
                    .map(|f| f.id),
            )
            .collect();
        let mut wishes: Vec<(ItemId, DeskPos)> = Vec::new();
        for id in &sources {
            wishes.extend(self.shown_cells(*id));
        }
        for id in sources.iter().skip(1) {
            tracing::info!(fence = %id, "loose-icon fence becomes free icons");
            self.delete_fence(*id);
            changed = true;
        }
        for (item, pos) in wishes {
            // Exactly where they were: no snapping on the way in.
            if self.config.fence_of_item(self.layout, item) != Some(inbox) {
                let layout = self.layout;
                self.config
                    .assign(layout, item, inbox, AssignedBy::Migration);
            }
            self.set_desk(item, Some(pos));
            changed = true;
        }
        if changed {
            self.mark_dirty();
        }
        changed
    }

    /// The cells `fence`'s members without a desktop place occupied in its window as earlier
    /// versions drew it (title row, padding, icon grid), as free icon cells.
    fn shown_cells(&self, fence: FenceId) -> Vec<(ItemId, DeskPos)> {
        let Some(f) = self.fence(fence) else {
            return Vec::new();
        };
        let mut refs: Vec<&pecofence_core::ItemRef> =
            f.items.iter().filter(|r| r.desk.is_none()).collect();
        if refs.is_empty() {
            return Vec::new();
        }
        refs.sort_by_key(|r| r.manual_index.unwrap_or(u32::MAX));
        let Some(work) = pecofence_core::geometry::resolve_work_area(&f.geometry, &self.work_areas)
        else {
            return Vec::new();
        };
        let s = work.scale();
        let px = self.fence_px_rect(f);
        let metrics =
            crate::layout::GridMetrics::for_icon_size(f.view.icon_size, f.view.label_lines)
                .with_spacing(f.view.spacing);
        let w = px.width() as f32 / s;
        let cols = f
            .view
            .columns
            .map(|c| c.max(1) as usize)
            .unwrap_or_else(|| ((w / metrics.cell_w).floor() as usize).max(1));
        let pitch = w / cols as f32;
        let cw = self.desk_grid.cell_w;
        let x0 = (px.left - work.left) as f32 / s;
        let y0 = (px.top - work.top) as f32 / s + FENCE_TITLE_DIP + metrics.pad_y;
        refs.iter()
            .enumerate()
            .map(|(i, r)| {
                let (col, row) = (i % cols, i / cols);
                (
                    r.item_id,
                    DeskPos {
                        monitor: work.device_path.clone(),
                        x: x0 + col as f32 * pitch + (pitch - cw) / 2.0,
                        y: y0 + row as f32 * metrics.cell_h + metrics.icon_top - DESK_ICON_TOP_DIP,
                    },
                )
            })
            .collect()
    }
}
