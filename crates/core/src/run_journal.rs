//! Crash-loop guard (MistFences). Each start is recorded and marked clean at a normal exit. When
//! the last [`LIMIT`] runs all ended without a clean exit within [`WINDOW_SECS`], the next
//! start is a safe one: the fences show, but the Windows desktop icons are left visible, so a
//! build that keeps crashing can never leave the desktop without icons. A clean exit of any run
//! (the safe one included) ends the streak.

use serde::{Deserialize, Serialize};

/// Unclean ends in a row that make the next start a safe one.
pub const LIMIT: usize = 3;
/// How recent those runs must be (seconds before the new start).
pub const WINDOW_SECS: i64 = 10 * 60;
/// Runs kept in the file.
const KEPT: usize = 10;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    /// Unix seconds.
    pub started: i64,
    pub clean: bool,
    #[serde(default)]
    pub safe: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunJournal {
    pub runs: Vec<Run>,
}

impl RunJournal {
    /// The journal from its file's text; an unreadable file counts as empty.
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }

    pub fn to_text(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// How many runs in a row, newest first, ended uncleanly within the window before `now`.
    pub fn unclean_streak(&self, now: i64) -> usize {
        self.runs
            .iter()
            .rev()
            .take_while(|r| !r.clean && now - r.started <= WINDOW_SECS && r.started <= now)
            .count()
    }

    /// Records a start at `now`; true when it must be a safe start.
    pub fn start(&mut self, now: i64) -> bool {
        let safe = self.unclean_streak(now) >= LIMIT;
        self.runs.push(Run {
            started: now,
            clean: false,
            safe,
        });
        if self.runs.len() > KEPT {
            let extra = self.runs.len() - KEPT;
            self.runs.drain(..extra);
        }
        safe
    }

    /// Marks the newest run as ended normally.
    pub fn finish(&mut self) {
        if let Some(r) = self.runs.last_mut() {
            r.clean = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_quick_crashes_make_a_safe_start() {
        let mut j = RunJournal::default();
        assert!(!j.start(1000));
        assert!(!j.start(1060)); // the first run crashed
        assert!(!j.start(1120)); // two crashes
        assert!(
            j.start(1180),
            "three unclean runs in a row within ten minutes"
        );
        // The safe run crashes too: still safe.
        assert!(j.start(1240));
        // A clean exit ends the streak.
        j.finish();
        assert!(!j.start(1300));
    }

    #[test]
    fn old_crashes_and_clean_runs_do_not_count() {
        let mut j = RunJournal::default();
        j.start(0);
        j.start(100);
        j.start(200);
        // Twenty minutes later: those crashes are too old.
        assert!(!j.start(200 + 20 * 60));
        let mut j = RunJournal::default();
        j.start(0);
        j.start(10);
        j.finish();
        j.start(20);
        assert!(!j.start(30), "only one unclean run since the clean one");
    }

    #[test]
    fn the_file_round_trips_and_stays_short() {
        let mut j = RunJournal::default();
        for t in 0..25 {
            j.start(t);
        }
        assert_eq!(j.runs.len(), KEPT);
        assert_eq!(RunJournal::parse(&j.to_text()), j);
        assert_eq!(RunJournal::parse("not json"), RunJournal::default());
    }
}
