//! MistFences safe preview (`--mistfences-preview`). A named instance with its own
//! configuration and a sandbox folder of sample items that stands in for the Desktop, run next
//! to whatever organizes the real desktop (Fences 6) without touching it:
//!
//! - the platform lockdown (`pecofence_platform::safety`) refuses every change to Windows-wide
//!   state and every file change outside the sandbox, whoever asks (menus, CLI, Settings);
//! - the real Desktop, Public Desktop and special items (Recycle Bin, This PC) are never read;
//! - the Windows desktop icons, the startup entry, global shortcuts and desktop mouse watching
//!   stay off.
//!
//! Layout of the preview folder (`%LOCALAPPDATA%\MistFences\Preview` by default):
//!
//! ```text
//! Preview\
//!   Desktop\   the sandbox the fences show (sample shortcuts, files and folders)
//!   Targets\   text files the sample shortcuts open
//!   config\    config.json, backups, snapshots
//! ```

use pecofence_core::geometry::{self, PxRect, WorkArea};
use pecofence_core::rules::{RuleSet, Unmatched};
use pecofence_core::{Cond, Fence, FenceKind, Rule, Settings, Target, ThemeSetting, TitleAlign};
use std::path::{Path, PathBuf};

pub const ARG: &str = "--mistfences-preview";
pub const ROOT_ARG: &str = "--preview-root";

/// The folders of one preview.
#[derive(Clone, Debug)]
pub struct PreviewPaths {
    pub root: PathBuf,
    pub sandbox: PathBuf,
    pub targets: PathBuf,
    pub config: PathBuf,
}

/// `%LOCALAPPDATA%\MistFences\Preview`.
pub fn default_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|base| base.join(pecofence_core::brand::NAME).join("Preview"))
}

/// A sample shortcut: file name, icon library in System32, icon index.
struct Sample {
    name: &'static str,
    icon_dll: &'static str,
    icon_index: u32,
}

const GAMES: [Sample; 6] = [
    sample("Космический симулятор", "ddores.dll", 29),
    sample("Песочница", "imageres.dll", 187),
    sample("Звёздные гонки", "imageres.dll", 204),
    sample("VR-арена", "ddores.dll", 22),
    sample("Ритм-игра", "imageres.dll", 103),
    sample("Онлайн-шутер", "ddores.dll", 8),
];

const PROGRAMS: [Sample; 6] = [
    sample("Браузер", "imageres.dll", 20),
    sample("Редактор кода", "ddores.dll", 27),
    sample("Видеоплеер", "imageres.dll", 178),
    sample("Музыка", "imageres.dll", 80),
    sample("Мониторинг", "imageres.dll", 144),
    sample("Настройки", "imageres.dll", 109),
];

const fn sample(name: &'static str, icon_dll: &'static str, icon_index: u32) -> Sample {
    Sample {
        name,
        icon_dll,
        icon_index,
    }
}

/// The preview's look: a custom theme (see docs/THEMES.md) close to Fences 6's default fences,
/// a dark plate that lets a little of the wallpaper through. Not the final MistFences design.
pub const THEME: &str = "MistFences Dark";
const THEME_JSON: &str = r##"{
  "base": "fluent",
  "blur": 24,
  "chroma": 0.4,
  "cornerRadius": 4,
  "shadowOpacity": 0.35,
  "dark": { "tint": "#141416E0", "luminosity": 0.85 },
  "light": { "tint": "#141416E0", "luminosity": 0.85 }
}
"##;

/// Plain sample files: name and contents. Folders end in `\`.
const FILES: [(&str, &str); 6] = [
    (
        "Заметки.txt",
        "Тестовый файл MistFences. Его можно открывать и переименовывать.\r\n",
    ),
    (
        "План проекта.md",
        "# План\r\n\r\n- Этап 1: безопасный предпросмотр\r\n",
    ),
    ("Бюджет.csv", "статья;сумма\r\nтест;0\r\n"),
    ("Настройки.json", "{ \"test\": true }\r\n"),
    ("Документы\\", ""),
    ("Фото\\", ""),
];

/// Creates the preview folders and the sample items that are missing (an existing item is
/// left alone, so renames the user made in the sandbox survive). Refuses a root inside the
/// real Desktop folders: the samples would appear on the desktop the running Fences shows.
pub fn prepare(root: &Path, real_desktops: &[PathBuf]) -> std::io::Result<PreviewPaths> {
    if !root.is_absolute() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "the preview folder must be an absolute path",
        ));
    }
    std::fs::create_dir_all(root)?;
    let root = canonical(root);
    for desktop in real_desktops {
        let desktop = canonical(desktop);
        if is_within(&root, &desktop) || is_within(&desktop, &root) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "the preview folder {} overlaps the Desktop folder {}",
                    root.display(),
                    desktop.display()
                ),
            ));
        }
    }
    let paths = PreviewPaths {
        sandbox: root.join("Desktop"),
        targets: root.join("Targets"),
        config: root.join("config"),
        root,
    };
    for dir in [&paths.sandbox, &paths.targets, &paths.config] {
        std::fs::create_dir_all(dir)?;
    }
    let system = std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join("System32");
    for s in GAMES.iter().chain(&PROGRAMS) {
        let target = paths.targets.join(format!("{}.txt", s.name));
        if !target.exists() {
            std::fs::write(
                &target,
                format!(
                    "\u{FEFF}«{}» — тестовый элемент MistFences, а не настоящая программа.\r\n",
                    s.name
                ),
            )?;
        }
        let link = paths.sandbox.join(format!("{}.url", s.name));
        if !link.exists() {
            write_url_shortcut(&link, &target, &system.join(s.icon_dll), s.icon_index)?;
        }
    }
    write_theme(&paths.config)?;
    for (name, text) in FILES {
        match name.strip_suffix('\\') {
            Some(folder) => std::fs::create_dir_all(paths.sandbox.join(folder))?,
            None => {
                let file = paths.sandbox.join(name);
                if !file.exists() {
                    std::fs::write(file, text)?;
                }
            }
        }
    }
    Ok(paths)
}

/// Puts the [`THEME`] file into the `themes` folder of the configuration in `config_dir`
/// unless one is there already (the user may have edited it).
pub fn write_theme(config_dir: &Path) -> std::io::Result<()> {
    let themes = config_dir.join("themes");
    std::fs::create_dir_all(&themes)?;
    let theme = themes.join(format!("{THEME}.json"));
    if !theme.exists() {
        std::fs::write(theme, THEME_JSON)?;
    }
    Ok(())
}

/// An Internet shortcut (`.url`) that opens `target` and shows icon `index` of `icon`.
/// Written as UTF-16 with a BOM, which Windows reads for non-ASCII names.
fn write_url_shortcut(path: &Path, target: &Path, icon: &Path, index: u32) -> std::io::Result<()> {
    let text = format!(
        "[InternetShortcut]\r\nURL={}\r\nIconFile={}\r\nIconIndex={index}\r\n",
        file_url(target),
        icon.display()
    );
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    std::fs::write(path, bytes)
}

/// `file:///C:/Users/%D0%90.../x.txt`: UTF-8 percent-encoded.
fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    let mut url = String::from("file:///");
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"/:-_.~".contains(&b) {
            url.push(b as char);
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// The settings every preview run keeps, whatever its configuration says: nothing that
/// changes Windows (desktop icons, startup entry) or takes input from other programs (the
/// Peek shortcut, double-click on the desktop). A fresh preview also starts dark with
/// centred titles, like the fences it imitates. Returns whether anything changed.
pub fn apply_settings(settings: &mut Settings, fresh: bool) -> bool {
    let before = settings.clone();
    settings.hide_real_icons = false;
    settings.autostart = false;
    settings.peek.enabled = false;
    settings.quick_hide.enabled = false;
    // Without hidden Windows icons the inbox would get no window: keep it like a real run.
    settings.hide_inbox_when_icons_shown = false;
    if fresh {
        settings.theme = ThemeSetting::Dark;
        settings.custom_theme = Some(THEME.to_string());
        settings.title_align = TitleAlign::Center;
        settings.roll_up.hover_peek = true;
        settings.roll_up.click_to_expand = false;
    }
    *settings != before
}

/// The work area the preview fences start on: the primary monitor's.
fn primary(work_areas: &[WorkArea]) -> WorkArea {
    work_areas
        .iter()
        .find(|w| w.mon_left == 0 && w.mon_top == 0)
        .or(work_areas.first())
        .cloned()
        .unwrap_or(WorkArea {
            device_path: "primary".into(),
            gdi_name: String::new(),
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1040,
            dpi: 96,
            mon_left: 0,
            mon_top: 0,
            mon_right: 1920,
            mon_bottom: 1080,
        })
}

/// The first preview layout: 「Игры」, 「Программы」 and 「Файлы」 of different sizes, rolled up
/// to their titles across the middle of the primary monitor (clear of the fences Fences keeps
/// along the top edge), plus the inbox for anything new. Rules file the samples by name.
pub fn default_layout(work_areas: &[WorkArea], icon_size: u32) -> (Vec<Fence>, RuleSet) {
    let work = primary(work_areas);
    let (ww, wh) = (work.width_dip(), work.height_dip());
    // Fractions of the work area so a small screen still fits them; DIPs otherwise.
    let y = (wh * 0.42).min(600.0);
    let specs: [(&str, f32, f32, f32); 3] = [
        ("Игры", 0.20, 520.0, 300.0),
        ("Программы", 0.44, 420.0, 260.0),
        ("Файлы", 0.64, 380.0, 320.0),
    ];
    let px = |x: f32, y: f32, w: f32, h: f32| {
        let s = work.scale();
        PxRect {
            left: work.left + (x * s) as i32,
            top: work.top + (y * s) as i32,
            right: work.left + ((x + w) * s) as i32,
            bottom: work.top + ((y + h) * s) as i32,
        }
    };
    let mut fences = Vec::new();
    let mut ids = Vec::new();
    for (title, fx, w, h) in specs {
        let w = w.min(ww * 0.3);
        let h = h.min(wh - y - 16.0).max(120.0);
        let geo = geometry::normalize(px(ww * fx, y, w, h), &work);
        let mut f = Fence::new(title, FenceKind::Virtual, geo);
        f.view.icon_size = icon_size;
        f.rolled_up = true;
        ids.push(f.id);
        fences.push(f);
    }
    let inbox_geo = geometry::normalize(px(ww * 0.82, y, 300.0f32.min(ww * 0.16), 220.0), &work);
    let mut inbox = Fence::new("Неразобранное", FenceKind::Inbox, inbox_geo);
    inbox.view.icon_size = icon_size;
    inbox.hide_when_empty = true;
    fences.push(inbox);

    let names = |samples: &[Sample]| -> Vec<String> {
        samples.iter().map(|s| format!("{}.url", s.name)).collect()
    };
    let files: Vec<String> = FILES
        .iter()
        .map(|(name, _)| name.trim_end_matches('\\').to_string())
        .collect();
    let rules = RuleSet {
        default_target: Target::Inbox,
        keep_updated: true,
        unmatched: Unmatched::DefaultTarget,
        list: vec![
            Rule::new(
                "Тест: игры",
                Target::Fence(ids[0]),
                vec![Cond::ExactName(names(&GAMES))],
            ),
            Rule::new(
                "Тест: программы",
                Target::Fence(ids[1]),
                vec![Cond::ExactName(names(&PROGRAMS))],
            ),
            Rule::new(
                "Тест: файлы",
                Target::Fence(ids[2]),
                vec![Cond::ExactName(files)],
            ),
        ],
    };
    (fences, rules)
}

fn canonical(path: &Path) -> PathBuf {
    let real = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = real.to_string_lossy();
    PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text))
}

/// Whether `inner` is `outer` or below it (case ignored).
fn is_within(inner: &Path, outer: &Path) -> bool {
    let inner = inner.to_string_lossy().to_lowercase();
    let outer = outer.to_string_lossy().to_lowercase();
    let outer = outer.trim_end_matches('\\');
    inner == outer || inner.starts_with(&format!("{outer}\\"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mistfences-preview-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn prepare_creates_the_samples_once_and_keeps_user_changes() {
        let root = scratch("prepare");
        let paths = prepare(&root, &[]).unwrap();
        let count = std::fs::read_dir(&paths.sandbox).unwrap().count();
        assert_eq!(count, GAMES.len() + PROGRAMS.len() + FILES.len());
        let notes = paths.sandbox.join("Заметки.txt");
        std::fs::write(&notes, "edited").unwrap();
        prepare(&root, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&notes).unwrap(), "edited");
        let url = std::fs::read(paths.sandbox.join("Браузер.url")).unwrap();
        assert_eq!(&url[..2], &[0xFF, 0xFE]);
        let text = String::from_utf16(
            &url[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        assert!(
            text.contains("URL=file:///") && text.contains("IconIndex=20"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_refuses_a_root_inside_or_around_a_desktop() {
        let root = scratch("desktop");
        let desktop = root.join("Desktop of someone");
        std::fs::create_dir_all(&desktop).unwrap();
        assert!(prepare(&desktop.join("Preview"), std::slice::from_ref(&desktop)).is_err());
        assert!(prepare(&root, std::slice::from_ref(&desktop)).is_err());
        assert!(prepare(Path::new("relative"), &[]).is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn file_urls_are_percent_encoded() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\Тест\a b.txt")),
            "file:///C:/Users/%D0%A2%D0%B5%D1%81%D1%82/a%20b.txt"
        );
    }

    #[test]
    fn preview_settings_never_touch_windows() {
        let mut s = Settings {
            hide_real_icons: true,
            autostart: true,
            ..Settings::default()
        };
        s.peek.enabled = true;
        s.quick_hide.enabled = true;
        assert!(apply_settings(&mut s, false));
        assert!(!s.hide_real_icons && !s.autostart && !s.peek.enabled && !s.quick_hide.enabled);
        assert_eq!(
            s.title_align,
            TitleAlign::Left,
            "only a fresh preview restyles"
        );
        assert!(
            !apply_settings(&mut s, false),
            "a second pass changes nothing"
        );
        assert!(apply_settings(&mut s, true));
        assert_eq!(s.theme, ThemeSetting::Dark);
        assert_eq!(s.title_align, TitleAlign::Center);
    }

    #[test]
    fn default_layout_has_three_rolled_fences_and_routes_every_sample() {
        let work = primary(&[]);
        let (fences, rules) = default_layout(&[work], 48);
        let titles: Vec<&str> = fences.iter().map(|f| f.title.as_str()).collect();
        assert_eq!(titles, ["Игры", "Программы", "Файлы", "Неразобранное"]);
        assert!(fences[..3].iter().all(|f| f.rolled_up));
        assert!(fences[3].hide_when_empty);
        assert_eq!(rules.list.len(), 3);
        let sizes: Vec<(u32, u32)> = fences[..3]
            .iter()
            .map(|f| (f.geometry.w as u32, f.geometry.h as u32))
            .collect();
        assert_ne!(sizes[0], sizes[1]);
        assert_ne!(sizes[1], sizes[2]);
    }
}
