//! Free desktop icons in the App (MistFences): keeps `desktop_layer` in step with the state,
//! reads Windows' desktop grid, keeps the icons on it, and puts icons dropped on the bare
//! desktop where they were dropped.

use super::*;
use crate::desktop_layer::DeskItem;
use crate::free_icons::DESK_ICON_TOP_DIP;
use pecofence_core::DeskPos;
use pecofence_core::desk::DeskGrid;

/// Icons dropped from a fence go down a column from the drop point, this many per column.
const DROP_COLUMN_ROWS: usize = 5;

impl App {
    /// Windows' desktop grid in DIPs from Explorer's list view (its icon spacing at the system
    /// DPI); the built-in 75 x 99 when Explorer does not answer.
    pub(super) fn read_desk_grid(&mut self) {
        let def_view = self
            .anchor
            .borrow()
            .as_ref()
            .and_then(|a| a.host())
            .map(|h| h.def_view);
        // The anchor may not have found Explorer's desktop yet at start: look it up then.
        let def_view = def_view.or_else(|| {
            pecofence_platform::desktop::resolve_icon_host(
                pecofence_platform::desktop::detect_generation(),
            )
            .map(|h| h.def_view)
        });
        let grid = def_view.and_then(pecofence_platform::shell_icons::explorer_grid);
        let dpi = monitors::system_dpi().max(96) as f32;
        let mut desk = DeskGrid::default();
        if let Some(g) = grid {
            desk.cell_w = g.spacing.0 as f32 * 96.0 / dpi;
            desk.cell_h = g.spacing.1 as f32 * 96.0 / dpi;
            if let Some((x, y)) = g.origin {
                desk.origin_x = x as f32 * 96.0 / dpi;
                desk.origin_y = y as f32 * 96.0 / dpi;
            }
        }
        tracing::info!(
            spacing = ?grid.map(|g| g.spacing),
            explorer_origin = ?grid.and_then(|g| g.origin),
            explorer_snaps = ?grid.map(|g| g.snap_to_grid),
            cell = ?(desk.cell_w, desk.cell_h),
            origin = ?(desk.origin_x, desk.origin_y),
            "desktop grid"
        );
        self.state.desk_grid = desk;
    }

    /// With "align to grid" on, free icons that are off the grid (saved by an earlier version,
    /// or dropped while it was off) are lined up on it; a snapshot taken first keeps their old
    /// places (Settings › Snapshots, or `snapshot restore`).
    pub(super) fn align_desk_if_needed(&mut self) {
        if !self.state.config.settings.desk_snap_to_grid {
            return;
        }
        let off = self.state.free_icons_off_grid();
        if off == 0 {
            return;
        }
        let snapshot = self
            .state
            .save_snapshot(pecofence_core::i18n::text("桌面图标对齐网格之前"));
        let moved = self.state.align_free_icons();
        tracing::info!(off, moved, %snapshot, "free icons lined up on the desktop grid");
        self.schedule_save();
    }

    /// Places new free icons, then shows every free icon where it is. Hidden while the Windows
    /// icons show (they include these items), except in the preview, whose sandbox items
    /// Explorer does not show.
    pub(super) fn sync_desk(&mut self) {
        if self.state.place_free_items() {
            self.schedule_save();
        }
        let Some(inbox) = self.state.inbox_id() else {
            return;
        };
        let icons: Vec<_> = self
            .state
            .free_icons()
            .into_iter()
            .filter_map(|fi| {
                let it = self.state.item(fi.item)?;
                let item = DeskItem {
                    id: it.id,
                    label: it.display_name.clone(),
                    path: PathBuf::from(it.path()?),
                    icon: it.icon_key.clone(),
                };
                Some((fi, item))
            })
            .collect();
        let visible = (self.state.config.settings.hide_real_icons && !self.no_hide_icons)
            || pecofence_platform::safety::preview_active();
        let icon_size = self
            .state
            .fence(inbox)
            .map_or(self.state.config.settings.icon_size, |f| f.view.icon_size);
        self.desk.sync(
            icons,
            &self.state.work_areas,
            self.state.desk_grid,
            inbox,
            icon_size,
            visible,
        );
    }

    /// `items` were dropped on the bare desktop at screen point `at`: they become free icons
    /// there. Free icons dragged together keep their places relative to the one held
    /// (`grab`); icons from a fence go down a column from the drop point.
    pub(super) fn place_on_desktop(
        &mut self,
        items: Vec<ItemId>,
        at: (i32, i32),
        grab: Option<(ItemId, i32, i32)>,
    ) {
        let Some(work) = self.desk_work_area(at) else {
            return;
        };
        let s = work.scale();
        let grid = self.state.desk_grid;
        let anchor_px = match grab {
            Some((_, ox, oy)) => (at.0 - ox, at.1 - oy),
            // The pointer on the icon's middle, as Explorer drops a dragged file.
            None => (
                at.0 - (grid.cell_w * s / 2.0).round() as i32,
                at.1 - ((DESK_ICON_TOP_DIP + 24.0) * s).round() as i32,
            ),
        };
        let ax = (anchor_px.0 - work.left) as f32 / s;
        let ay = (anchor_px.1 - work.top) as f32 / s;
        let free = self.state.free_icons();
        let held = grab.and_then(|(id, _, _)| free.iter().find(|f| f.item == id).cloned());
        let mut column = 0usize;
        let wishes: Vec<(ItemId, DeskPos)> = items
            .iter()
            .map(|id| {
                let kept = held.as_ref().and_then(|h| {
                    free.iter()
                        .find(|f| f.item == *id && f.monitor == h.monitor)
                        .map(|f| (ax + (f.x - h.x), ay + (f.y - h.y)))
                });
                let (x, y) = kept.unwrap_or_else(|| {
                    let i = column;
                    column += 1;
                    (
                        ax + (i / DROP_COLUMN_ROWS) as f32 * grid.cell_w,
                        ay + (i % DROP_COLUMN_ROWS) as f32 * grid.cell_h,
                    )
                });
                (
                    *id,
                    DeskPos {
                        monitor: work.device_path.clone(),
                        x,
                        y,
                    },
                )
            })
            .collect();
        let placed = self.state.place_on_desktop(&wishes);
        tracing::info!(items = items.len(), placed, "icons placed on the desktop");
        if placed > 0 {
            self.refresh_all();
            self.desk.select(&items);
            self.schedule_save();
        }
    }

    /// Files from another program were dropped on the desktop at `at`: once the desktop has
    /// them (the shell may still be writing), they become free icons there.
    pub(super) fn place_new_on_desktop(&mut self, names: Vec<String>, at: (i32, i32)) {
        let _ = self.sync_desktop_if_available("drop on the desktop");
        let Some(desk) = shell::user_desktop() else {
            return;
        };
        let items: Vec<ItemId> = names
            .iter()
            .filter_map(|n| self.state.item_id_for_path(&desk.join(n)))
            .collect();
        tracing::info!(
            names = names.len(),
            found = items.len(),
            "dropped files placed"
        );
        if !items.is_empty() {
            self.place_on_desktop(items, at, None);
        }
    }

    fn desk_work_area(&self, at: (i32, i32)) -> Option<pecofence_core::geometry::WorkArea> {
        self.state
            .work_areas
            .iter()
            .find(|w| {
                at.0 >= w.mon_left && at.0 < w.mon_right && at.1 >= w.mon_top && at.1 < w.mon_bottom
            })
            .or(self.state.work_areas.first())
            .cloned()
    }
}
