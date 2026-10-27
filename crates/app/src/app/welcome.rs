//! The first start's welcome: a notice in the corner (`crate::notice`, 30 s) says the files
//! stay where they are and how to make and hide fences, and its button opens the user guide
//! (also on Settings › 「关于」).

use super::*;

impl App {
    pub(super) fn offer_welcome(&mut self) {
        match crate::notice::Notice::show(
            &self.ctx,
            pecofence_core::i18n::text("桌面整理好了"),
            pecofence_core::i18n::text(
                "文件都没有移动，只是显示在栅栏里。\n在桌面空白处用右键拖出一个框可新建栅栏，双击空白处可隐藏或显示栅栏。",
            ),
            pecofence_core::i18n::text("使用手册"),
            crate::notice::locale(),
        ) {
            Ok(notice) => self.welcome = Some(notice),
            Err(error) => tracing::warn!(%error, "welcome notice not shown"),
        }
    }

    /// MistFences' user guide (Russian and English) in its repository.
    pub(super) fn open_manual(&self) {
        let url = format!(
            "{}/blob/main/docs/mistfences/USER-GUIDE.md",
            pecofence_core::brand::REPOSITORY_URL
        );
        let _ = shell::shell_execute(Path::new(&url), None, None);
    }
}
