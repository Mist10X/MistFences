//! The grid free desktop icons sit on (MistFences): Windows 11's own desktop grid, so an icon
//! dropped on the desktop lands in a cell as it does in Explorer with "Align icons to grid" on.
//! Positions are DIPs from the top-left corner of a monitor's work area; a position is the
//! top-left corner of the icon's cell (Explorer draws the 48 px icon centred in the cell's
//! width, 2 px below its top, and the label under it).

use crate::model::DESK_CELL_DIP;

/// Explorer's desktop grid in DIPs: the icon spacing (`LVM_GETITEMSPACING` of its list view,
/// 75 x 99 at 100 % with medium icons) and where the first cell starts in a work area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeskGrid {
    pub cell_w: f32,
    pub cell_h: f32,
    pub origin_x: f32,
    pub origin_y: f32,
}

impl Default for DeskGrid {
    fn default() -> Self {
        Self {
            cell_w: DESK_CELL_DIP.0,
            cell_h: DESK_CELL_DIP.1,
            origin_x: 0.0,
            origin_y: 0.0,
        }
    }
}

/// A rectangle in DIPs within a work area (a fence as shown there).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DipRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl DipRect {
    fn overlaps(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        x < self.x + self.w && self.x < x + w && y < self.y + self.h && self.y < y + h
    }
}

impl DeskGrid {
    /// Columns and rows in a work area of `work_w` x `work_h` DIPs (at least one each).
    pub fn size(&self, work_w: f32, work_h: f32) -> (usize, usize) {
        (
            (((work_w - self.origin_x) / self.cell_w).floor() as usize).max(1),
            (((work_h - self.origin_y) / self.cell_h).floor() as usize).max(1),
        )
    }

    fn cell_at(&self, col: usize, row: usize) -> (f32, f32) {
        (
            self.origin_x + col as f32 * self.cell_w,
            self.origin_y + row as f32 * self.cell_h,
        )
    }

    /// The grid cell (its top-left corner) nearest to a cell wanted at `x`, `y`.
    pub fn snap(&self, x: f32, y: f32, work_w: f32, work_h: f32) -> (f32, f32) {
        let (cols, rows) = self.size(work_w, work_h);
        let col = ((x - self.origin_x) / self.cell_w)
            .round()
            .clamp(0.0, (cols - 1) as f32) as usize;
        let row = ((y - self.origin_y) / self.cell_h)
            .round()
            .clamp(0.0, (rows - 1) as f32) as usize;
        self.cell_at(col, row)
    }

    /// Whether `x`, `y` is a cell of this grid (to a tenth of a DIP).
    pub fn is_on_grid(&self, x: f32, y: f32) -> bool {
        let fx = (x - self.origin_x) / self.cell_w;
        let fy = (y - self.origin_y) / self.cell_h;
        (fx - fx.round()).abs() * self.cell_w < 0.1 && (fy - fy.round()).abs() * self.cell_h < 0.1
    }

    /// Whether a cell at `x`, `y` is taken: another icon's cell overlaps most of it (icons
    /// placed off the grid count by overlap), or a fence covers part of it.
    pub fn is_taken(&self, x: f32, y: f32, taken: &[(f32, f32)], blocked: &[DipRect]) -> bool {
        taken.iter().any(|&(tx, ty)| {
            (tx - x).abs() < self.cell_w * 0.75 && (ty - y).abs() < self.cell_h * 0.75
        }) || blocked
            .iter()
            .any(|b| b.overlaps(x + 2.0, y + 2.0, self.cell_w - 4.0, self.cell_h - 4.0))
    }

    /// A free grid cell: the one nearest to `want`, or with `want` None the first in
    /// Explorer's order (down the first column, then the next column). None when every cell
    /// is taken.
    pub fn free_cell(
        &self,
        work_w: f32,
        work_h: f32,
        taken: &[(f32, f32)],
        blocked: &[DipRect],
        want: Option<(f32, f32)>,
    ) -> Option<(f32, f32)> {
        let (cols, rows) = self.size(work_w, work_h);
        let mut best: Option<((f32, f32), f32)> = None;
        for col in 0..cols {
            for row in 0..rows {
                let (x, y) = self.cell_at(col, row);
                if self.is_taken(x, y, taken, blocked) {
                    continue;
                }
                let Some((wx, wy)) = want else {
                    return Some((x, y));
                };
                let d = (x - wx).powi(2) + (y - wy).powi(2);
                if best.is_none_or(|(_, bd)| d < bd) {
                    best = Some(((x, y), d));
                }
            }
        }
        best.map(|(cell, _)| cell)
    }

    /// Keeps a cell wholly inside the work area (a position saved on a larger screen).
    pub fn clamp_into(&self, x: f32, y: f32, work_w: f32, work_h: f32) -> (f32, f32) {
        (
            x.clamp(0.0, (work_w - self.cell_w).max(0.0)),
            y.clamp(0.0, (work_h - self.cell_h).max(0.0)),
        )
    }

    /// Lines icons placed anywhere up on the grid: each takes the free cell nearest to where it
    /// is, in reading order (top to bottom, left to right), so a row stays a row and no two
    /// icons share a cell. `icons` are (x, y) positions; the result has the same order.
    pub fn align(&self, icons: &[(f32, f32)], work_w: f32, work_h: f32) -> Vec<(f32, f32)> {
        let mut order: Vec<usize> = (0..icons.len()).collect();
        order.sort_by(|&a, &b| {
            let (ax, ay) = icons[a];
            let (bx, by) = icons[b];
            // Rows first, on the grid's own row height (icons a few DIPs apart share a row).
            let ra = ((ay - self.origin_y) / self.cell_h).round();
            let rb = ((by - self.origin_y) / self.cell_h).round();
            ra.total_cmp(&rb).then(ax.total_cmp(&bx))
        });
        let mut placed: Vec<(f32, f32)> = Vec::with_capacity(icons.len());
        let mut out = vec![(0.0, 0.0); icons.len()];
        for i in order {
            let (x, y) = icons[i];
            let (sx, sy) = self.snap(x, y, work_w, work_h);
            let cell = if self.is_taken(sx, sy, &placed, &[]) {
                self.free_cell(work_w, work_h, &placed, &[], Some((sx, sy)))
                    .unwrap_or((sx, sy))
            } else {
                (sx, sy)
            };
            placed.push(cell);
            out[i] = cell;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: DeskGrid = DeskGrid {
        cell_w: 75.0,
        cell_h: 99.0,
        origin_x: 0.0,
        origin_y: 0.0,
    };

    #[test]
    fn snaps_to_explorers_grid() {
        assert_eq!(G.snap(975.0, 495.0, 2560.0, 1392.0), (975.0, 495.0));
        assert_eq!(G.snap(1000.0, 530.0, 2560.0, 1392.0), (975.0, 495.0));
        assert_eq!(G.snap(1020.0, 560.0, 2560.0, 1392.0), (1050.0, 594.0));
        // Never off the screen.
        assert_eq!(G.snap(-500.0, 9000.0, 2560.0, 1392.0), (0.0, 13.0 * 99.0));
        assert!(G.is_on_grid(1050.0, 594.0));
        assert!(!G.is_on_grid(1063.0, 594.0));
    }

    #[test]
    fn free_cells_skip_icons_and_fences() {
        let taken = [(0.0, 0.0), (0.0, 99.0)];
        // Explorer's order: down the first column.
        assert_eq!(
            G.free_cell(800.0, 600.0, &taken, &[], None),
            Some((0.0, 198.0))
        );
        let fence = DipRect {
            x: 0.0,
            y: 0.0,
            w: 150.0,
            h: 390.0,
        };
        // The first column below the fence comes before the next column.
        assert_eq!(
            G.free_cell(800.0, 600.0, &[], &[fence], None),
            Some((0.0, 396.0))
        );
        // Near its top, the column beside it.
        assert_eq!(
            G.free_cell(800.0, 600.0, &[], &[fence], Some((150.0, 0.0))),
            Some((150.0, 0.0))
        );
        let all = DipRect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 600.0,
        };
        assert_eq!(G.free_cell(800.0, 600.0, &[], &[all], None), None);
    }

    #[test]
    fn off_grid_icons_still_take_their_cell() {
        assert!(G.is_taken(0.0, 0.0, &[(10.0, 10.0)], &[]));
        assert!(!G.is_taken(75.0, 0.0, &[(10.0, 10.0)], &[]));
    }

    /// The two rows of the user's screenshot: four icons 7 DIPs lower than the rest end up in
    /// one row, one column each, nobody on top of anybody.
    #[test]
    fn aligning_turns_ragged_rows_into_grid_rows() {
        let icons = [
            (905.0, 405.0),
            (980.0, 405.0),
            (1055.0, 405.0),
            (1130.0, 405.0),
            (1205.0, 398.0),
            (1280.0, 398.0),
            (905.0, 493.0),
            (980.0, 493.0),
        ];
        let out = G.align(&icons, 2560.0, 1392.0);
        let first_row: Vec<f32> = out[..6].iter().map(|c| c.1).collect();
        assert!(first_row.iter().all(|y| *y == first_row[0]), "{out:?}");
        assert_eq!(out[6].1, out[7].1);
        assert!(out[6].1 > first_row[0]);
        for c in &out {
            assert!(G.is_on_grid(c.0, c.1), "{c:?}");
        }
        let mut cells = out.clone();
        cells.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        cells.dedup();
        assert_eq!(cells.len(), out.len(), "no two icons share a cell");
        // Columns keep their order and stay one cell apart.
        assert_eq!(out[1].0 - out[0].0, 75.0);
    }

    #[test]
    fn aligning_moves_a_collision_to_the_nearest_free_cell() {
        // Two icons that snap to the same cell.
        let out = G.align(&[(70.0, 0.0), (80.0, 0.0)], 800.0, 600.0);
        assert_ne!(out[0], out[1]);
        assert!(out.iter().all(|c| G.is_on_grid(c.0, c.1)));
    }
}
