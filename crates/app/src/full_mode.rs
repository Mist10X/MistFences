//! Safeguards of a normal run, the one that manages the real Desktop (MistFences full mode):
//!
//! - it does not start while Fences 6 manages the desktop (two programs would draw and hide the
//!   same icons);
//! - after several crashes in a row it starts safe: fences yes, Windows icons left visible
//!   (`pecofence_core::run_journal`).
//!
//! The preview, the review and the import never get here.

use pecofence_core::run_journal::RunJournal;
use std::path::PathBuf;

/// `--check-fences`: no window, exit code 0 when Fences is not active, 10 when it is, written
/// to `<folder>\fences-check.txt` too when `--config-dir <folder>` is given. For the switch
/// scripts.
pub const CHECK_ARG: &str = "--check-fences";
pub const CHECK_ACTIVE: i32 = 10;

/// `--desk-report --config-dir <folder>`: no window; loads the configuration in `<folder>` as a
/// start would (migrations included, so run it on a copy), saves it, and writes
/// `<folder>\desk-report.txt` with the fences and where each free desktop icon sits.
pub const DESK_REPORT_ARG: &str = "--desk-report";

pub fn desk_report(dir: &std::path::Path) -> i32 {
    use std::fmt::Write as _;
    let mut state = crate::state::AppState::load_at(crate::app::work_areas(), dir);
    state.place_free_items();
    state.mark_dirty();
    let saved = state.save_if_dirty();
    let mut r = String::new();
    let _ = writeln!(r, "saved: {saved}");
    for w in &state.work_areas {
        let _ = writeln!(
            r,
            "monitor {} work {},{} {}x{} dpi {}",
            w.device_path,
            w.left,
            w.top,
            w.right - w.left,
            w.bottom - w.top,
            w.dpi
        );
    }
    for f in state.fences() {
        let px = state.fence_px_rect(f);
        let _ = writeln!(
            r,
            "fence {:?} {:?} items {} at {},{} {}x{} rolled {}",
            f.title,
            f.kind,
            f.items.len(),
            px.left,
            px.top,
            px.width(),
            px.height(),
            f.rolled_up
        );
    }
    for fi in state.free_icons() {
        let name = state
            .item(fi.item)
            .map(|it| it.display_name.clone())
            .unwrap_or_default();
        let _ = writeln!(r, "free {:.1},{:.1} {} {:?}", fi.x, fi.y, fi.monitor, name);
    }
    match std::fs::write(dir.join("desk-report.txt"), r) {
        Ok(()) if saved => 0,
        _ => 1,
    }
}

/// Lines describing what was found, empty when Fences is not active.
pub fn fences_report(found: &pecofence_platform::fences_detect::FencesPresence) -> Vec<String> {
    found
        .explorer_modules
        .iter()
        .chain(&found.processes)
        .cloned()
        .collect()
}

/// Refuses the start while Fences is active: tells the user and returns false.
pub fn fences_allow_start() -> bool {
    let found = pecofence_platform::fences_detect::detect();
    if !found.unreadable.is_empty() {
        tracing::warn!(pids = ?found.unreadable, "Explorer modules could not be listed");
    }
    if !found.active() {
        return true;
    }
    let lines = fences_report(&found);
    tracing::error!(
        found = lines.len(),
        "Fences is active; MistFences does not start"
    );
    pecofence_platform::window::alert(
        &format!(
            "Stardock Fences сейчас управляет рабочим столом:\n\n{}\n\n\
             MistFences не запускается вместе с ним, чтобы две программы не скрывали и не \
             рисовали одни и те же значки. Сначала закройте или удалите Fences (раздел «Переход со \
             Stardock Fences» в README.md рядом с программой).",
            lines.join("\n")
        ),
        "MistFences",
    );
    false
}

fn journal_path(instance: Option<&str>) -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let name = match instance {
        Some(n) => format!("runs.{n}.json"),
        None => "runs.json".to_string(),
    };
    Some(base.join(pecofence_core::brand::NAME).join(name))
}

/// The run journal of this instance, with this start recorded.
pub struct Journal {
    path: Option<PathBuf>,
    journal: RunJournal,
    /// This start is a safe one (see the module doc).
    pub safe: bool,
}

impl Journal {
    pub fn start(instance: Option<&str>) -> Self {
        let path = journal_path(instance);
        let mut journal = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|t| RunJournal::parse(&t))
            .unwrap_or_default();
        let safe = journal.start(pecofence_core::now_unix());
        let me = Self {
            path,
            journal,
            safe,
        };
        me.write();
        if safe {
            tracing::warn!(
                limit = pecofence_core::run_journal::LIMIT,
                "crash loop: this start leaves the Windows icons visible"
            );
        }
        me
    }

    /// A normal exit: the streak ends.
    pub fn finish(mut self) {
        self.journal.finish();
        self.write();
    }

    fn write(&self) {
        if let Some(p) = &self.path {
            if let Some(dir) = p.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(p, self.journal.to_text());
        }
    }
}
