//! Import from Stardock Fences 6 (MistFences). Turns a Fences snapshot (`DailySnapshotN.xml`)
//! into a MistFences configuration in a folder of its own:
//!
//! `mistfences.exe --import-fences <snapshot.xml> --config-dir <folder> [--dry-run] [--replace]`
//!
//! Runs without windows and changes nothing but that folder: it reads a copy of the snapshot,
//! the monitors and the Desktop listing, and never moves, renames or deletes a desktop item, nor
//! touches Fences, its settings or Explorer. `--dry-run` writes only the report (what would be
//! imported, and everything that cannot be). Without it the configuration is written, read
//! back and checked; an existing `config.json` is kept (the import refuses) unless `--replace`
//! moves it aside first.
//!
//! What maps to what:
//! - each Fences group → a fence with the same title, monitor (by device id, not by the
//!   `\\.\DISPLAYn` number Windows reassigns), position and size (clamped to the work area),
//!   and its icons in Fences' order (row by row);
//! - roll state: Fences saves none. A group docked to the top edge while "roll up when
//!   docked" is on shows rolled up, so such a group is imported rolled (reported as inferred);
//! - loose icons → transparent fences where they were, one per cluster; the largest becomes
//!   the inbox (`Рабочий стол`) that also receives every new desktop item;
//! - items in the snapshot that are no longer on the Desktop are listed, not invented; items on
//!   the Desktop the snapshot does not know go to the inbox and are listed too.

use crate::state::AppState;
use pecofence_core::fences6::{self, Snapshot};
use pecofence_core::geometry::{self, PxRect, WorkArea};
use pecofence_core::{
    AppearanceOverride, AssignedBy, ConfigStore, Fence, FenceKind, ItemKey, LoadOutcome, RuleSet,
    SortMode, Spacing, ThemeSetting, TitleAlign,
};
use pecofence_platform::shell::{self, DesktopEntry};
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const ARG: &str = "--import-fences";
pub const CONFIG_ARG: &str = "--config-dir";
pub const DRY_RUN_ARG: &str = "--dry-run";
pub const REPLACE_ARG: &str = "--replace";

/// Height of a Fences title bar at 100 % (its `MetricsPerMonitor`); MistFences' themes use the
/// same 36 DIPs, so a fence's title lands where the group's was.
const FENCES_TITLE_PX: f32 = 36.0;
/// Windows' desktop icon grid at 100 % when the snapshot does not record it.
const DESKTOP_CELL: (i32, i32) = (75, 99);
/// MistFences' icon cell at 48 px icons (see `layout::GridMetrics`): 80 × 96 DIPs, 8 DIPs of
/// padding above and below the grid.
const CELL_DIP: (f32, f32) = (80.0, 96.0);
const GRID_PAD_DIP: f32 = 8.0;
/// The compact cell's height at 48 px icons (`Spacing::Compact`), used for loose icons.
const COMPACT_CELL_H_DIP: f32 = 88.0;
pub const LOOSE_TITLE: &str = "Рабочий стол";

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// A Fences group, by its id.
    Group(i32),
    /// Loose desktop icons; `inbox` for the one that also takes new items.
    Loose { inbox: bool },
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedItem {
    /// The path as on disk (`::{CLSID}` for the Recycle Bin and This PC).
    pub path: String,
    pub open_count: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlannedFence {
    pub title: String,
    pub source: Source,
    /// `WorkArea::device_path` of the monitor.
    pub monitor: String,
    /// Expanded rectangle, virtual-screen px.
    pub rect: PxRect,
    pub rolled: bool,
    pub transparent: bool,
    /// Loose icons: Windows' desktop grid, this many columns of compact cells.
    pub desktop_columns: Option<u32>,
    pub items: Vec<PlannedItem>,
    pub notes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MissingIcon {
    pub path: String,
    /// The group title, or "вне групп".
    pub group: String,
}

#[derive(Debug, Default)]
pub struct Plan {
    /// Snapshot monitor → the connected monitor it is, as report lines.
    pub monitors: Vec<String>,
    pub fences: Vec<PlannedFence>,
    pub missing: Vec<MissingIcon>,
    /// On the Desktop now, not in the snapshot: they land in the inbox.
    pub new_items: Vec<String>,
    pub settings: Vec<String>,
    pub not_imported: Vec<String>,
    /// Anything that makes the import unsafe to apply.
    pub problems: Vec<String>,
}

impl Plan {
    pub fn item_count(&self) -> usize {
        self.fences.iter().map(|f| f.items.len()).sum()
    }
}

/// Settings the import takes over, read from the snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedSettings {
    pub icon_size: u32,
    pub roll_up: bool,
    pub quick_hide: bool,
}

pub fn imported_settings(snap: &Snapshot) -> ImportedSettings {
    ImportedSettings {
        icon_size: snap
            .setting_int("IconSizeLast")
            .filter(|s| (16..=256).contains(s))
            .map_or(48, |s| s as u32),
        roll_up: snap.setting_int("RollupEnabled").unwrap_or(1) != 0,
        quick_hide: snap.setting_int("QuickHideEnabled").unwrap_or(0) != 0,
    }
}

fn label(w: &WorkArea) -> String {
    let [sw, sh] = w.screen_px();
    format!(
        "{} {}×{} at ({}, {})",
        w.model().unwrap_or(&w.device_path),
        sw,
        sh,
        w.mon_left,
        w.mon_top
    )
}

/// The connected monitor a snapshot monitor is: same device instance, else same model and
/// resolution, else the same rectangle.
fn find_area<'a>(m: &fences6::Monitor, areas: &'a [WorkArea]) -> Option<&'a WorkArea> {
    let id = m.device_id.as_deref();
    let model = id.and_then(geometry::monitor_model);
    areas
        .iter()
        .find(|w| id.is_some_and(|id| w.device_path.eq_ignore_ascii_case(id)))
        .or_else(|| {
            areas.iter().find(|w| {
                model.is_some_and(|mm| w.model().is_some_and(|wm| wm.eq_ignore_ascii_case(mm)))
                    && w.screen_px() == [m.width, m.height]
            })
        })
        .or_else(|| {
            areas.iter().find(|w| {
                w.mon_left == m.left && w.mon_top == m.top && w.screen_px() == [m.width, m.height]
            })
        })
}

/// Keeps `r` inside `w`'s work area: moved in first, shrunk only when larger. Returns the
/// clamped rectangle and what changed, for the report.
fn clamp_into(mut r: PxRect, w: &WorkArea) -> (PxRect, Vec<String>) {
    let mut notes = Vec::new();
    let (ww, wh) = (w.right - w.left, w.bottom - w.top);
    if r.bottom > w.bottom && r.top >= w.top && r.top < w.bottom - wh / 4 {
        // Taller than the room below its top: keep the top where the user put it and end at
        // the bottom of the work area (the rest scrolls, as it did in Fences).
        notes.push(format!(
            "высота {} px не помещается на монитор, уменьшена до {} px (остальное прокручивается)",
            r.height(),
            w.bottom - r.top
        ));
        r.bottom = w.bottom;
    } else if r.height() > wh {
        notes.push(format!(
            "высота {} px не помещается на монитор, уменьшена до {} px (остальное прокручивается)",
            r.height(),
            wh
        ));
        r.bottom = r.top + wh;
    }
    if r.width() > ww {
        notes.push(format!("ширина {} px уменьшена до {} px", r.width(), ww));
        r.right = r.left + ww;
    }
    let dy = if r.top < w.top {
        w.top - r.top
    } else if r.bottom > w.bottom {
        w.bottom - r.bottom
    } else {
        0
    };
    let dx = if r.left < w.left {
        w.left - r.left
    } else if r.right > w.right {
        w.right - r.right
    } else {
        0
    };
    if dx != 0 || dy != 0 {
        notes.push(format!(
            "сдвинута на ({dx}, {dy}) px внутрь рабочей области"
        ));
        r.left += dx;
        r.right += dx;
        r.top += dy;
        r.bottom += dy;
    }
    (r, notes)
}

/// `75x99` from `SystemIconMetricsLast` (`96|96|96|48|75x99|…`): Windows' desktop grid.
fn desktop_cell(snap: &Snapshot) -> (i32, i32) {
    snap.setting("SystemIconMetricsLast")
        .and_then(|v| v.split('|').nth(4))
        .and_then(|c| c.split_once('x'))
        .and_then(|(a, b)| Some((a.trim().parse().ok()?, b.trim().parse().ok()?)))
        .filter(|&(a, b): &(i32, i32)| a > 0 && b > 0)
        .unwrap_or(DESKTOP_CELL)
}

/// Builds the import plan. Pure: `areas` are the connected monitors, `entries` the Desktop
/// listing (user and public folders plus the Recycle Bin / This PC items).
pub fn build_plan(snap: &Snapshot, areas: &[WorkArea], entries: &[DesktopEntry]) -> Plan {
    let mut plan = Plan::default();
    let settings = imported_settings(snap);
    let on_desktop: HashMap<ItemKey, &DesktopEntry> = entries
        .iter()
        .map(|e| (ItemKey::from_path(&e.path.to_string_lossy()), e))
        .collect();
    let present = |path: &str| on_desktop.get(&ItemKey::from_path(path)).copied();

    // Monitors.
    let mut mapped: Vec<(&fences6::Monitor, Option<&WorkArea>)> = Vec::new();
    for m in &snap.monitors {
        let area = find_area(m, areas);
        plan.monitors.push(format!(
            "{} {}×{} at ({}, {}) [{}] → {}",
            m.device_id.as_deref().unwrap_or("?"),
            m.width,
            m.height,
            m.left,
            m.top,
            m.gdi_name,
            area.map_or_else(|| "не подключён".to_string(), label)
        ));
        if let Some(w) = area
            && w.screen_px() != [m.width, m.height]
        {
            plan.not_imported.push(format!(
                "монитор {} сейчас {}×{}, в снимке {}×{}: позиции перенесены как есть и прижаты к экрану",
                label(w),
                w.screen_px()[0],
                w.screen_px()[1],
                m.width,
                m.height
            ));
        }
        mapped.push((m, area));
    }
    if snap.monitors.is_empty() {
        plan.problems
            .push("в снимке нет списка мониторов (ResolutionFingerprintEx)".to_string());
    }
    let area_of = |m: &fences6::Monitor| {
        mapped
            .iter()
            .find(|(sm, _)| std::ptr::eq(*sm, m))
            .and_then(|(_, a)| *a)
    };
    let rollup_on_dock = settings.roll_up && snap.setting_int("RollupOnDock").unwrap_or(0) != 0;

    // Groups.
    for g in &snap.groups {
        let Some(m) = snap.monitor_of(g) else {
            plan.problems.push(format!(
                "группа «{}»: монитор {} не найден в снимке",
                g.title, g.gdi_name
            ));
            continue;
        };
        let Some(w) = area_of(m) else {
            plan.problems.push(format!(
                "группа «{}»: её монитор {} сейчас не подключён",
                g.title,
                m.device_id.as_deref().unwrap_or(&m.gdi_name)
            ));
            continue;
        };
        let scale = m.scale_pct as f32 / 100.0;
        let title = (FENCES_TITLE_PX * scale).round() as i32;
        let (dx, dy) = (w.mon_left - m.left, w.mon_top - m.top);
        let rect = PxRect {
            left: g.x + dx,
            top: g.y - title + dy,
            right: g.x + g.w + dx,
            bottom: g.y + g.h + dy,
        };
        let (rect, mut notes) = clamp_into(rect, w);
        let docked = g.y - title <= m.top + 1;
        let rolled = rollup_on_dock && docked;
        if rolled {
            notes.push(
                "свёрнута — выведено: группа прикреплена к верхнему краю, а в Fences включено «сворачивать при прикреплении»"
                    .to_string(),
            );
        }
        if g.scroll != 0 {
            notes.push(format!(
                "позиция прокрутки ({}) не переносится, группа откроется с начала",
                g.scroll
            ));
        }
        let mut items = Vec::new();
        for icon in snap.icons_of(g.id) {
            match present(&icon.path) {
                Some(e) => items.push(PlannedItem {
                    path: e.path.to_string_lossy().into_owned(),
                    open_count: icon.open_count,
                }),
                None => plan.missing.push(MissingIcon {
                    path: icon.path.clone(),
                    group: g.title.clone(),
                }),
            }
        }
        plan.fences.push(PlannedFence {
            title: g.title.clone(),
            source: Source::Group(g.id),
            monitor: w.device_path.clone(),
            rect,
            rolled,
            transparent: false,
            desktop_columns: None,
            items,
            notes,
        });
    }
    for icon in &snap.icons {
        if icon.group >= 0 && snap.group(icon.group).is_none() && !icon.is_desktop_ini() {
            plan.missing.push(MissingIcon {
                path: icon.path.clone(),
                group: format!("группа {} (её нет в снимке)", icon.group),
            });
        }
    }

    // Loose icons: clusters on Windows' desktop grid, per monitor.
    let cell = desktop_cell(snap);
    struct Loose<'a> {
        x: i32,
        y: i32,
        monitor: &'a fences6::Monitor,
        item: PlannedItem,
    }
    let mut loose: Vec<Loose> = Vec::new();
    for icon in snap.icons_of(-1) {
        let Some(e) = present(&icon.path) else {
            plan.missing.push(MissingIcon {
                path: icon.path.clone(),
                group: "вне групп".to_string(),
            });
            continue;
        };
        let Some(monitor) = snap
            .monitor_at(icon.abs_x, icon.abs_y)
            .or(snap.monitors.first())
        else {
            continue;
        };
        loose.push(Loose {
            x: icon.abs_x,
            y: icon.abs_y,
            monitor,
            item: PlannedItem {
                path: e.path.to_string_lossy().into_owned(),
                open_count: icon.open_count,
            },
        });
    }
    // Union-find over neighbours (a cell apart at most, diagonals included).
    let mut parent: Vec<usize> = (0..loose.len()).collect();
    fn root(p: &mut [usize], mut i: usize) -> usize {
        while p[i] != i {
            p[i] = p[p[i]];
            i = p[i];
        }
        i
    }
    for i in 0..loose.len() {
        for j in i + 1..loose.len() {
            let (a, b) = (&loose[i], &loose[j]);
            if std::ptr::eq(a.monitor, b.monitor)
                && (a.x - b.x).abs() <= cell.0 * 3 / 2
                && (a.y - b.y).abs() <= cell.1 * 3 / 2
            {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    let mut index_of: HashMap<usize, usize> = HashMap::new();
    for i in 0..loose.len() {
        let r = root(&mut parent, i);
        let c = *index_of.entry(r).or_insert_with(|| {
            clusters.push(Vec::new());
            clusters.len() - 1
        });
        clusters[c].push(i);
    }
    for c in &mut clusters {
        c.sort_by_key(|&i| (loose[i].y, loose[i].x));
    }
    // The largest cluster is the inbox; then top to bottom, left to right.
    clusters.sort_by_key(|c| {
        let first = &loose[c[0]];
        (std::cmp::Reverse(c.len()), first.y, first.x)
    });
    let mut loose_fences = Vec::new();
    for (n, c) in clusters.iter().enumerate() {
        let m = loose[c[0]].monitor;
        let Some(w) = area_of(m) else {
            for &i in c {
                plan.problems.push(format!(
                    "значок {} вне групп: его монитор сейчас не подключён",
                    loose[i].item.path
                ));
            }
            continue;
        };
        let min_x = c.iter().map(|&i| loose[i].x).min().unwrap_or(0);
        let max_x = c.iter().map(|&i| loose[i].x).max().unwrap_or(0);
        let min_y = c.iter().map(|&i| loose[i].y).min().unwrap_or(0);
        let max_y = c.iter().map(|&i| loose[i].y).max().unwrap_or(0);
        let cols = ((max_x - min_x) / cell.0 + 1).max(1);
        let rows = ((max_y - min_y) / cell.1 + 1).max(1);
        // Windows' own grid: compact cells, one column per desktop cell, so each icon stays in
        // its column (rows are MistFences' compact height, a little lower than Windows').
        let s = w.scale();
        let (dx, dy) = (w.mon_left - m.left, w.mon_top - m.top);
        let title = (FENCES_TITLE_PX * s).round() as i32;
        let pad = (GRID_PAD_DIP * s).round() as i32;
        let left = min_x + dx;
        let top = min_y + dy - title - pad;
        let rect = PxRect {
            left,
            top,
            right: left + cols * cell.0,
            bottom: top + title + pad * 2 + rows * (COMPACT_CELL_H_DIP * s).round() as i32,
        };
        let (rect, mut notes) = clamp_into(rect, w);
        if (cols * rows) as usize > c.len() {
            notes.push(format!(
                "значки стояли с пропусками в сетке {cols}×{rows}: порядок сохранён, пропуски нет"
            ));
        }
        let inbox = n == 0;
        if inbox {
            notes.push("сюда же попадают новые значки рабочего стола".to_string());
        }
        loose_fences.push(PlannedFence {
            title: if inbox {
                LOOSE_TITLE.to_string()
            } else {
                format!("{LOOSE_TITLE} {}", n + 1)
            },
            source: Source::Loose { inbox },
            monitor: w.device_path.clone(),
            rect,
            rolled: false,
            transparent: true,
            desktop_columns: Some(cols as u32),
            items: c.iter().map(|&i| loose[i].item.clone()).collect(),
            notes,
        });
    }
    if loose_fences.is_empty() {
        // Exactly one inbox per layout: an empty one, hidden until something lands on the
        // desktop, in the middle of the primary monitor.
        if let Some(w) = areas.first() {
            let s = w.scale();
            let (cw, ch) = ((CELL_DIP.0 * 4.0 * s) as i32, (180.0 * s) as i32);
            let (cx, cy) = ((w.left + w.right) / 2, (w.top + w.bottom) / 2);
            loose_fences.push(PlannedFence {
                title: LOOSE_TITLE.to_string(),
                source: Source::Loose { inbox: true },
                monitor: w.device_path.clone(),
                rect: PxRect {
                    left: cx - cw / 2,
                    top: cy - ch / 2,
                    right: cx + cw / 2,
                    bottom: cy + ch / 2,
                },
                rolled: false,
                transparent: true,
                desktop_columns: None,
                items: Vec::new(),
                notes: vec![
                    "пустая, появляется, когда на рабочем столе появляется новый значок"
                        .to_string(),
                ],
            });
        }
    }
    plan.fences.extend(loose_fences);

    // Desktop items the snapshot does not know.
    let known: std::collections::HashSet<ItemKey> = snap
        .icons
        .iter()
        .map(|i| ItemKey::from_path(&i.path))
        .collect();
    let mut new_items: Vec<String> = entries
        .iter()
        .filter(|e| !known.contains(&ItemKey::from_path(&e.path.to_string_lossy())))
        .map(|e| e.path.to_string_lossy().into_owned())
        .collect();
    new_items.sort();
    plan.new_items = new_items;

    // Settings.
    plan.settings = vec![
        format!("размер значков: {} px (IconSizeLast)", settings.icon_size),
        format!(
            "сворачивание групп: {} — свёрнутая группа раскрывается сразу при наведении на заголовок и сворачивается, когда курсор уходит",
            if settings.roll_up { "включено" } else { "выключено" }
        ),
        "соседние группы при раскрытии не сдвигаются (как в Fences)".to_string(),
        format!(
            "Quick Hide (скрытие двойным щелчком по рабочему столу): {}",
            if settings.quick_hide { "включено" } else { "выключено, как в Fences" }
        ),
        "заголовки по центру, тёмная тема «MistFences Dark»".to_string(),
        "значки Windows скрываются, пока работает MistFences (полный режим), и возвращаются при выходе".to_string(),
        "автозапуск выключен — включается только после вашего согласия".to_string(),
        "Peek (Ctrl+Alt+Space) выключен — можно включить в настройках".to_string(),
    ];
    for (name, target) in &snap.rules {
        plan.not_imported.push(if *target < 0 {
            format!("правило Fences {name}: не переносится, в Fences оно никуда не направляет файлы (цель −1)")
        } else {
            format!(
                "правило Fences {name} → группа {target}: не переносится автоматически, его нужно настроить заново"
            )
        });
    }
    for (name, why) in [
        (
            "DesktopGrid",
            "сетка рабочего стола Windows: у MistFences своя сетка в группах",
        ),
        (
            "DesktopAutoArrange",
            "упорядочивание значков Windows: не используется",
        ),
        (
            "DesktopSorting",
            "сортировка рабочего стола Windows: в группах порядок ручной, как был",
        ),
    ] {
        if snap.setting(name).is_some() {
            plan.not_imported.push(format!("настройка {name}: {why}"));
        }
    }
    plan.not_imported.push(
        "раскладки Fences для других разрешений (PerMonitorConfig, например при смене разрешения в игре) не переносятся: MistFences сам держит группы на месте при смене разрешения".to_string(),
    );
    plan.not_imported.push(
        "сетка Fences 91×91 px против 80×96 px у MistFences: в широкой группе в ряд может поместиться другое число значков, порядок тот же".to_string(),
    );
    plan.not_imported.push(
        "значки вне групп стоят в тех же колонках сетки Windows (75 px), но шаг рядов у MistFences 88 px вместо 99: каждый следующий ряд на 11 px выше прежнего места".to_string(),
    );
    for s in &snap.skipped {
        plan.not_imported
            .push(format!("{}: «{}» — {}", s.section, s.text, s.reason));
    }
    plan
}

// ---- applying ----------------------------------------------------------------------------

/// Replaces the fences of `state`'s current layout with the planned ones, files the Desktop
/// items into them in order and takes over the settings. `state` should be a fresh
/// configuration (`AppState::load_at` on an empty folder).
pub fn apply(plan: &Plan, state: &mut AppState, entries: &[DesktopEntry], icon_size: u32) {
    let mut fences = Vec::new();
    for pf in &plan.fences {
        let Some(work) = state
            .work_areas
            .iter()
            .find(|w| w.device_path == pf.monitor)
            .cloned()
        else {
            continue;
        };
        let geo = geometry::normalize(pf.rect, &work);
        let kind = match pf.source {
            Source::Loose { inbox: true } => FenceKind::Inbox,
            _ => FenceKind::Virtual,
        };
        let mut f = Fence::new(&pf.title, kind, geo);
        f.rolled_up = pf.rolled;
        f.view.icon_size = icon_size;
        f.view.sort = SortMode::Manual;
        if pf.transparent {
            f.view.auto_height = true;
            if let Some(cols) = pf.desktop_columns {
                f.view.spacing = Spacing::Compact;
                f.view.columns = Some(cols);
            }
            f.appearance = Some(AppearanceOverride {
                opacity: Some(0.0),
                title_on_hover: Some(true),
                ..Default::default()
            });
            f.hide_when_empty = pf.items.is_empty();
        }
        fences.push(f);
    }
    let layout = state.layout;
    state.config.layouts[layout].fences = fences;
    // Fences' rules filed nothing (see the plan): new items simply go to the inbox.
    state.config.rules = RuleSet::default();
    state.config.undo_log.clear();
    state.mark_dirty();
    state.sync_desktop(entries);

    let fence_ids: Vec<_> = state.config.layouts[layout]
        .fences
        .iter()
        .map(|f| f.id)
        .collect();
    let opened: HashMap<String, u32> = plan
        .fences
        .iter()
        .flat_map(|f| &f.items)
        .map(|i| (i.path.to_lowercase(), i.open_count))
        .collect();
    for (pf, &fid) in plan.fences.iter().zip(&fence_ids) {
        let mut order = Vec::new();
        for it in &pf.items {
            let Some(id) = state.item_id_for_path(Path::new(&it.path)) else {
                continue;
            };
            state.config.assign(layout, id, fid, AssignedBy::Migration);
            order.push(id);
        }
        if let Some(f) = state.fence_mut(fid) {
            // The planned order first, then anything else routed here (new items).
            let mut next = order.len() as u32;
            for r in &mut f.items {
                r.manual_index = Some(match order.iter().position(|id| *id == r.item_id) {
                    Some(i) => i as u32,
                    None => {
                        next += 1;
                        next - 1
                    }
                });
            }
        }
    }
    for item in state.config.items.values_mut() {
        if let Some(path) = item.path()
            && let Some(&n) = opened.get(&path.to_lowercase())
        {
            item.open_count = n;
        }
    }

    let s = &mut state.config.settings;
    s.icon_size = icon_size;
    s.roll_up.hover_peek = true;
    s.roll_up.click_to_expand = false;
    s.roll_up.push_neighbors = false;
    s.quick_hide.enabled = false;
    s.peek.enabled = false;
    s.title_align = TitleAlign::Center;
    s.theme = ThemeSetting::Dark;
    s.custom_theme = Some(crate::preview::THEME.to_string());
    s.hide_real_icons = true;
    s.autostart = false;
    s.desktop_path = shell::user_desktop().map(|d| d.to_string_lossy().into_owned());
    state.mark_dirty();
}

/// Reads the saved configuration back and compares it with the plan: titles, monitors, roll
/// state and every fence's items in order. Returns the mismatches (empty = all as planned).
pub fn verify(plan: &Plan, dir: &Path) -> Vec<String> {
    let (outcome, _) = ConfigStore::new(dir).load_reporting();
    let config = match outcome {
        LoadOutcome::Primary(c) => c,
        _ => return vec!["сохранённая конфигурация не читается".to_string()],
    };
    let mut errors = Vec::new();
    let Some(layout) = config.layouts.iter().max_by_key(|l| l.shown) else {
        return vec!["в конфигурации нет раскладки".to_string()];
    };
    if layout.fences.len() != plan.fences.len() {
        errors.push(format!(
            "групп {} вместо {}",
            layout.fences.len(),
            plan.fences.len()
        ));
    }
    for pf in &plan.fences {
        let Some(f) = layout.fences.iter().find(|f| f.title == pf.title) else {
            errors.push(format!("нет группы «{}»", pf.title));
            continue;
        };
        if f.geometry.monitor != pf.monitor {
            errors.push(format!("«{}» на другом мониторе", pf.title));
        }
        if f.rolled_up != pf.rolled {
            errors.push(format!("«{}»: другое состояние свёрнутости", pf.title));
        }
        let mut refs = f.items.clone();
        refs.sort_by_key(|r| r.manual_index.unwrap_or(u32::MAX));
        let got: Vec<String> = refs
            .iter()
            .filter_map(|r| config.items.get(&r.item_id))
            .filter_map(|i| i.path().map(str::to_lowercase))
            .collect();
        let want: Vec<String> = pf.items.iter().map(|i| i.path.to_lowercase()).collect();
        if !got.starts_with(&want) {
            errors.push(format!(
                "«{}»: значки или их порядок не совпадают ({} из {})",
                pf.title,
                got.iter().zip(&want).take_while(|(a, b)| a == b).count(),
                want.len()
            ));
        }
        if matches!(pf.source, Source::Group(_)) && got.len() != want.len() {
            errors.push(format!(
                "«{}»: {} значков вместо {}",
                pf.title,
                got.len(),
                want.len()
            ));
        }
    }
    errors
}

// ---- report ------------------------------------------------------------------------------

pub fn report(
    plan: &Plan,
    snap: &Snapshot,
    source: &Path,
    copy: &Path,
    applied: Option<&[String]>,
) -> String {
    let mut r = String::new();
    let _ = writeln!(
        r,
        "# Импорт из Fences 6 — {}\n",
        match applied {
            None => "предварительный просмотр (ничего не записано)",
            Some([]) => "выполнен и проверен",
            Some(_) => "выполнен, проверка нашла расхождения",
        }
    );
    let _ = writeln!(r, "- Снимок: `{}`", source.display());
    let _ = writeln!(r, "- Прочитана копия: `{}`", copy.display());
    if let Some(ts) = &snap.timestamp {
        let _ = writeln!(r, "- Время снимка: {ts}");
    }
    let _ = writeln!(
        r,
        "- Групп Fences: {}; значков в группах: {}; вне групп: {}",
        snap.groups.len(),
        snap.icons
            .iter()
            .filter(|i| i.group >= 0 && !i.is_desktop_ini())
            .count(),
        snap.icons_of(-1).len()
    );
    let _ = writeln!(
        r,
        "- Реальные файлы не перемещаются, не переименовываются и не удаляются.\n"
    );

    let _ = writeln!(r, "## Мониторы\n");
    for m in &plan.monitors {
        let _ = writeln!(r, "- {m}");
    }

    if !plan.problems.is_empty() {
        let _ = writeln!(r, "\n## Проблемы (импорт не применяется)\n");
        for p in &plan.problems {
            let _ = writeln!(r, "- {p}");
        }
    }

    let _ = writeln!(r, "\n## Группы\n");
    let _ = writeln!(
        r,
        "| # | Название | Монитор | Слева, сверху (px) | Ширина × высота (px) | Состояние | Значков |"
    );
    let _ = writeln!(r, "|---|---|---|---|---|---|---|");
    for (i, f) in plan.fences.iter().enumerate() {
        let _ = writeln!(
            r,
            "| {} | {}{} | {} | {}, {} | {} × {} | {} | {} |",
            i + 1,
            f.title,
            match f.source {
                Source::Group(_) => "",
                Source::Loose { inbox: true } => " (вне групп, принимает новые)",
                Source::Loose { inbox: false } => " (вне групп)",
            },
            geometry::monitor_model(&f.monitor).unwrap_or(&f.monitor),
            f.rect.left,
            f.rect.top,
            f.rect.width(),
            f.rect.height(),
            if f.rolled {
                "свёрнута"
            } else if f.transparent {
                "прозрачная"
            } else {
                "раскрыта"
            },
            f.items.len()
        );
    }
    for f in &plan.fences {
        let _ = writeln!(r, "\n### {}\n", f.title);
        for n in &f.notes {
            let _ = writeln!(r, "> {n}");
        }
        if !f.notes.is_empty() {
            let _ = writeln!(r);
        }
        for (i, it) in f.items.iter().enumerate() {
            let _ = writeln!(r, "{}. {}", i + 1, display(&it.path));
        }
        if f.items.is_empty() {
            let _ = writeln!(r, "(пусто)");
        }
    }

    let _ = writeln!(
        r,
        "\n## Есть в снимке, но нет на рабочем столе ({})\n",
        plan.missing.len()
    );
    if plan.missing.is_empty() {
        let _ = writeln!(r, "Нет.");
    }
    for m in &plan.missing {
        let _ = writeln!(r, "- {} — из «{}»", m.path, m.group);
    }

    let _ = writeln!(
        r,
        "\n## Есть на рабочем столе, но нет в снимке → «{LOOSE_TITLE}» ({})\n",
        plan.new_items.len()
    );
    if plan.new_items.is_empty() {
        let _ = writeln!(r, "Нет.");
    }
    for p in &plan.new_items {
        let _ = writeln!(r, "- {}", display(p));
    }

    let _ = writeln!(r, "\n## Настройки\n");
    for s in &plan.settings {
        let _ = writeln!(r, "- {s}");
    }
    let _ = writeln!(r, "\n## Не переносится\n");
    for s in &plan.not_imported {
        let _ = writeln!(r, "- {s}");
    }
    if let Some(errors) = applied {
        let _ = writeln!(r, "\n## Проверка записанной конфигурации\n");
        if errors.is_empty() {
            let _ = writeln!(
                r,
                "Конфигурация прочитана заново: все {} групп(ы), их мониторы, свёрнутость и {} значков в нужном порядке совпадают с планом.",
                plan.fences.len(),
                plan.item_count()
            );
        }
        for e in errors {
            let _ = writeln!(r, "- {e}");
        }
    }
    r
}

/// A path for the report: the Recycle Bin and This PC by name.
fn display(path: &str) -> String {
    match path.to_ascii_uppercase().as_str() {
        "::{645FF040-5081-101B-9F08-00AA002F954E}" => "Корзина".to_string(),
        "::{20D04FE0-3AEA-1069-A2D8-08002B30309D}" => "Этот компьютер".to_string(),
        _ => path.to_string(),
    }
}

// ---- command line ------------------------------------------------------------------------

/// The import's own command line, when this run is one.
pub struct Request {
    pub snapshot: PathBuf,
    pub config_dir: PathBuf,
    pub dry_run: bool,
    pub replace: bool,
}

pub fn request(args: &[String]) -> Option<Result<Request, String>> {
    let at = args.iter().position(|a| a == ARG)?;
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
    };
    let Some(snapshot) = args.get(at + 1).map(PathBuf::from) else {
        return Some(Err(format!("{ARG} <snapshot.xml> is missing the file")));
    };
    let Some(config_dir) = value(CONFIG_ARG) else {
        return Some(Err(format!("{CONFIG_ARG} <folder> is required")));
    };
    if !snapshot.is_absolute() || !config_dir.is_absolute() {
        return Some(Err("paths must be absolute".to_string()));
    }
    Some(Ok(Request {
        snapshot,
        config_dir,
        dry_run: args.iter().any(|a| a == DRY_RUN_ARG),
        replace: args.iter().any(|a| a == REPLACE_ARG),
    }))
}

/// `yyyymmdd-hhmmss` (UTC) for file names.
fn stamp() -> String {
    let t = pecofence_core::now_unix();
    let (days, secs) = (t.div_euclid(86_400), t.rem_euclid(86_400));
    // Civil from days (Howard Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}-{:02}{:02}{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

/// Runs the import; the exit code: 0 done (or previewed), 1 bad request or I/O error,
/// 2 problems found (nothing written but the report), 3 a configuration exists already,
/// 4 the written configuration did not read back as planned.
pub fn run(req: &Request) -> i32 {
    match run_inner(req) {
        Ok(code) => code,
        Err(e) => {
            tracing::error!(error = %e, "Fences import failed");
            let _ = std::fs::create_dir_all(req.config_dir.join("import"));
            let _ = std::fs::write(
                req.config_dir.join("import").join("last-error.txt"),
                format!("{e}\r\n"),
            );
            1
        }
    }
}

fn run_inner(req: &Request) -> Result<i32, String> {
    let desktops: Vec<PathBuf> = [shell::user_desktop(), shell::public_desktop()]
        .into_iter()
        .flatten()
        .collect();
    let dir = &req.config_dir;
    for d in &desktops {
        let d = crate::state::canonical_path(d.clone());
        let target = crate::state::canonical_path(dir.clone());
        if target.starts_with(&d) {
            return Err(format!(
                "the configuration folder {} is inside the Desktop folder {}",
                dir.display(),
                d.display()
            ));
        }
    }
    let import_dir = dir.join("import");
    std::fs::create_dir_all(&import_dir).map_err(|e| format!("{}: {e}", import_dir.display()))?;
    let stamp = stamp();
    let stem = req
        .snapshot
        .file_stem()
        .map_or_else(|| "snapshot".into(), |s| s.to_string_lossy().into_owned());
    // Work from a copy: the original stays exactly as Fences (or the backup) left it.
    let copy = import_dir.join(format!("{stem}-{stamp}.xml"));
    std::fs::copy(&req.snapshot, &copy)
        .map_err(|e| format!("copying {}: {e}", req.snapshot.display()))?;
    let bytes = std::fs::read(&copy).map_err(|e| format!("{}: {e}", copy.display()))?;
    let snap = fences6::parse(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?;

    let areas = crate::app::work_areas();
    let (mut entries, unread) = shell::enumerate_desktop_reporting();
    entries.extend(shell::enumerate_special_desktop_items());
    let mut plan = build_plan(&snap, &areas, &entries);
    if !unread.is_empty() {
        plan.problems.push(format!(
            "папки рабочего стола прочитаны не полностью: {unread:?}"
        ));
    }
    tracing::info!(
        groups = plan.fences.len(),
        items = plan.item_count(),
        missing = plan.missing.len(),
        new = plan.new_items.len(),
        problems = plan.problems.len(),
        "Fences import planned"
    );
    let write_report = |text: &str, kind: &str| -> Result<PathBuf, String> {
        let path = import_dir.join(format!("{kind}-{stamp}.md"));
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        let _ = std::fs::write(import_dir.join(format!("{kind}-latest.md")), text);
        Ok(path)
    };
    if req.dry_run || !plan.problems.is_empty() {
        write_report(&report(&plan, &snap, &req.snapshot, &copy, None), "preview")?;
        return Ok(if plan.problems.is_empty() { 0 } else { 2 });
    }

    let config = ConfigStore::new(dir).primary_path();
    if config.exists() {
        if !req.replace {
            write_report(&report(&plan, &snap, &req.snapshot, &copy, None), "preview")?;
            return Ok(3);
        }
        let aside = dir.join(format!("config.before-import-{stamp}.json"));
        std::fs::rename(&config, &aside).map_err(|e| format!("{}: {e}", config.display()))?;
        tracing::info!(to = %aside.display(), "existing configuration moved aside");
    }
    let mut state = AppState::load_at(areas, dir);
    let icon_size = imported_settings(&snap).icon_size;
    apply(&plan, &mut state, &entries, icon_size);
    if !state.save_if_dirty() {
        return Err(format!("could not save {}", config.display()));
    }
    crate::preview::write_theme(dir).map_err(|e| format!("theme: {e}"))?;
    let errors = verify(&plan, dir);
    write_report(
        &report(&plan, &snap, &req.snapshot, &copy, Some(&errors)),
        "result",
    )?;
    Ok(if errors.is_empty() { 0 } else { 4 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pecofence_platform::shell::EntryOrigin;

    fn area(id: &str, left: i32, w: i32, h: i32, taskbar: i32) -> WorkArea {
        WorkArea {
            device_path: id.to_string(),
            gdi_name: String::new(),
            left,
            top: 0,
            right: left + w,
            bottom: h - taskbar,
            dpi: 96,
            mon_left: left,
            mon_top: 0,
            mon_right: left + w,
            mon_bottom: h,
        }
    }

    fn entry(path: &str) -> DesktopEntry {
        DesktopEntry {
            path: PathBuf::from(path),
            file_name: path.rsplit('\\').next().unwrap().to_string(),
            origin: if path.starts_with("::") {
                EntryOrigin::Namespace
            } else {
                EntryOrigin::UserDesktop
            },
            is_folder: false,
            attributes: 0,
            mtime: 0,
            size: 0,
            created: 0,
        }
    }

    const FP: &str = "[0|0|2560|1440|100|\\\\.\\DISPLAY2|x|\\\\?\\DISPLAY#AOC3202#5&amp;15dd3631&amp;0&amp;UID4355#{e6f07b5f}|r];[2560|0|1920|1080|100|\\\\.\\DISPLAY1|x|\\\\?\\DISPLAY#AOC2702#5&amp;15dd3631&amp;0&amp;UID4353#{e6f07b5f}|r];";

    fn snapshot(groups: &[&str], icons: &[(&str, &str)], settings: &[(&str, &str)]) -> Snapshot {
        let mut xml = format!(
            "<SnapshotInfo><ResolutionFingerprintEx>{FP}</ResolutionFingerprintEx><Groups><ArrayOfString><string /><string>{}</string></ArrayOfString>",
            groups.len()
        );
        for (i, g) in groups.iter().enumerate() {
            xml +=
                &format!("<ArrayOfString><string>{i}</string><string>{g}</string></ArrayOfString>");
        }
        xml += "</Groups><Icons>";
        for (p, v) in icons {
            xml +=
                &format!("<ArrayOfString><string>{p}</string><string>{v}</string></ArrayOfString>");
        }
        xml += "</Icons><Settings>";
        for (n, v) in settings {
            xml += &format!(
                "<ArrayOfString><string>{n}</string><string>System.Int32</string><string>&lt;int&gt;{v}&lt;/int&gt;</string></ArrayOfString>"
            );
        }
        xml += "</Settings></SnapshotInfo>";
        fences6::parse(&xml).unwrap()
    }

    fn two_monitors() -> Vec<WorkArea> {
        // GDI numbers swapped against the snapshot, as Windows does between sessions.
        vec![
            area("AOC3202#5&15DD3631&0&UID4355", 0, 2560, 1440, 48),
            area("AOC2702#5&15DD3631&0&UID4353", 2560, 1920, 1080, 48),
        ]
    }

    #[test]
    fn groups_keep_title_monitor_position_order_and_roll_state() {
        let snap = snapshot(
            &[
                r"3|889|36|910|457|0|Игры` |\\.\DISPLAY2` |0|0|2560|1440|0|0|255|0|` |-1|0|1|0|0|{BEA23395-174E-40E5-AC76-DCAFE96CD1F7} |0|0|0|",
                r"8|2591|43|910|1640|0|Учёба` |\\.\DISPLAY1` |2560|0|1920|1080|0|0|255|0|` |-1|0|1|0|0|{3716DA39-5E8A-47E6-9827-C923301EDA6E} |0|0|0|",
            ],
            &[
                (r"C:\D\b.url", r"3|1001|38|112|2|0|0|0|\\.\DISPLAY2|5|0"),
                (r"C:\D\a.url", r"3|910|38|21|2|0|0|0|\\.\DISPLAY2|0|0"),
                (r"C:\D\c.lnk", r"3|910|129|21|93|0|0|0|\\.\DISPLAY2|0|0"),
                (
                    r"C:\D\gone.lnk",
                    r"3|1092|129|112|93|0|0|0|\\.\DISPLAY2|0|0",
                ),
                (r"C:\D\s.lnk", r"8|2612|45|21|2|0|0|0|\\.\DISPLAY2|1|0"),
            ],
            &[
                ("RollupEnabled", "1"),
                ("RollupOnDock", "1"),
                ("IconSizeLast", "48"),
            ],
        );
        let entries: Vec<_> = [
            r"C:\D\A.url",
            r"C:\D\b.url",
            r"C:\D\c.lnk",
            r"C:\D\s.lnk",
            r"C:\D\new.txt",
        ]
        .iter()
        .map(|p| entry(p))
        .collect();
        let plan = build_plan(&snap, &two_monitors(), &entries);
        assert!(plan.problems.is_empty(), "{:?}", plan.problems);
        let games = &plan.fences[0];
        assert_eq!(games.title, "Игры");
        assert_eq!(games.monitor, "AOC3202#5&15DD3631&0&UID4355");
        assert_eq!(
            games.rect,
            PxRect {
                left: 889,
                top: 0,
                right: 1799,
                bottom: 493
            }
        );
        assert!(games.rolled, "docked to the top with roll-up on dock");
        // Row by row; the on-disk spelling of the path wins.
        let order: Vec<&str> = games.items.iter().map(|i| i.path.as_str()).collect();
        assert_eq!(order, [r"C:\D\A.url", r"C:\D\b.url", r"C:\D\c.lnk"]);
        assert_eq!(games.items[1].open_count, 5);
        assert_eq!(
            plan.missing,
            vec![MissingIcon {
                path: r"C:\D\gone.lnk".into(),
                group: "Игры".into()
            }]
        );
        let study = &plan.fences[1];
        assert_eq!(study.monitor, "AOC2702#5&15DD3631&0&UID4353");
        assert!(!study.rolled, "not docked: expanded");
        // 1640 px high on a 1032 px work area: clamped, from 7 px below the top.
        assert_eq!(
            study.rect,
            PxRect {
                left: 2591,
                top: 7,
                right: 3501,
                bottom: 1032
            }
        );
        assert!(study.notes.iter().any(|n| n.contains("уменьшена")));
        // No loose icons: an empty inbox is still planned.
        let inbox = plan.fences.last().unwrap();
        assert_eq!(inbox.source, Source::Loose { inbox: true });
        assert!(inbox.items.is_empty());
        assert_eq!(plan.new_items, vec![r"C:\D\new.txt".to_string()]);
    }

    #[test]
    fn loose_icons_become_clusters_with_the_largest_as_inbox() {
        let mut icons = Vec::new();
        let names: Vec<String> = (0..6).map(|i| format!(r"C:\D\m{i}.lnk")).collect();
        for (i, n) in names.iter().enumerate() {
            let (x, y) = if i < 4 {
                (988 + 75 * i as i32, 497)
            } else {
                (1138 + 75 * (i as i32 - 4), 596)
            };
            icons.push((
                n.as_str(),
                format!(r"-1|{x}|{y}|{x}|{y}|0|0|0|\\.\DISPLAY2|0|0"),
            ));
        }
        icons.push((
            r"C:\D\far.txt",
            r"-1|13|1289|13|1289|0|0|0|\\.\DISPLAY2|0|0".to_string(),
        ));
        icons.push((
            r"::{645FF040-5081-101B-9F08-00AA002F954E}",
            r"-1|1288|596|1288|596|0|0|0|\\.\DISPLAY2|58|0".to_string(),
        ));
        icons.push((
            r"C:\D\desktop.ini",
            r"-1|0|156|0|156|0|0|0|\\.\DISPLAY2`|0|0".to_string(),
        ));
        let icons: Vec<(&str, &str)> = icons.iter().map(|(a, b)| (*a, b.as_str())).collect();
        let snap = snapshot(&[], &icons, &[]);
        let mut entries: Vec<_> = names.iter().map(|p| entry(p)).collect();
        entries.push(entry(r"C:\D\far.txt"));
        entries.push(entry("::{645ff040-5081-101b-9f08-00aa002f954e}"));
        let plan = build_plan(&snap, &two_monitors(), &entries);
        assert_eq!(plan.fences.len(), 2);
        let inbox = &plan.fences[0];
        assert_eq!(inbox.title, LOOSE_TITLE);
        assert_eq!(inbox.source, Source::Loose { inbox: true });
        assert!(inbox.transparent);
        assert_eq!(inbox.items.len(), 7);
        // Reading order: the first row, then the second (Recycle Bin after m4, m5).
        assert_eq!(inbox.items[4].path, r"C:\D\m4.lnk");
        assert_eq!(
            inbox.items[6].path,
            "::{645ff040-5081-101b-9f08-00aa002f954e}"
        );
        assert_eq!(inbox.items[6].open_count, 58);
        // Its icons start where Fences' did: the title and padding sit above them.
        assert_eq!(inbox.rect.top, 497 - 36 - 8);
        // One column per Windows grid cell, starting at the first icon's cell.
        assert_eq!(inbox.desktop_columns, Some(5)); // x 988 ..= 1288 (the Recycle Bin)
        assert_eq!((inbox.rect.left, inbox.rect.width()), (988, 5 * 75));
        assert_eq!(plan.fences[1].title, format!("{LOOSE_TITLE} 2"));
        assert_eq!(plan.fences[1].items.len(), 1);
        assert!(plan.missing.is_empty() && plan.new_items.is_empty());
    }

    #[test]
    fn a_missing_monitor_is_a_problem_not_a_guess() {
        let snap = snapshot(
            &[r"8|2591|43|910|500|0|Учёба` |\\.\DISPLAY1` |2560|0|1920|1080|0|0|255|0|` |"],
            &[],
            &[],
        );
        let plan = build_plan(&snap, &two_monitors()[..1], &[]);
        assert_eq!(plan.problems.len(), 1);
        assert!(plan.problems[0].contains("Учёба"));
    }

    #[test]
    fn stamp_is_a_file_name() {
        let s = stamp();
        assert_eq!(s.len(), 16);
        assert!(s.ends_with('Z') && s.as_bytes()[8] == b'-');
    }
}
