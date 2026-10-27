//! PecoFence — open-source Fences-style desktop organizer for Windows 11.
// GUI subsystem: no console window when launched from Explorer. Logs go to a file (see
// `init_logging`); stderr is still used when a console is attached (e.g. `cargo run`).
#![windows_subsystem = "windows"]

mod anchor;
mod app;
mod commands;
mod desktop_layer;
mod drag_guides;
mod drop_preview;
mod fence_window;
mod fences_import;
mod free_icons;
mod full_mode;
mod icons;
mod ipc_server;
mod layout;
mod marquee_band;
mod notice;
mod peek;
mod peek_search;
mod preview;
mod rename;
mod settings_host;
mod shadow;
mod share_log;
mod state;

use pecofence_platform::com::OleGuard;
use pecofence_platform::window;
use windows_core::Result;

fn parse_args() -> app::Args {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    app::Args {
        light: has("--light"),
        dark: has("--dark"),
        wallpaper_override: value("--wallpaper"),
        portable: has("--portable"),
        no_hide_icons: has("--no-hide-icons"),
        exit_after_ms: value("--exit-after").and_then(|v| v.parse().ok()),
        dump_stats: has("--dump-stats"),
        open_settings: has("--open-settings"),
        portal: value("--portal"),
        test_script: value("--test-script"),
        instance: None,
        preview: None,
        review: false,
        config_dir: value(CONFIG_DIR_ARG).map(std::path::PathBuf::from),
        safe_start: false,
    }
}

/// `--config-dir <folder>`: the configuration lives there instead of %APPDATA%\MistFences (the
/// review always names one).
const CONFIG_DIR_ARG: &str = "--config-dir";
/// `--mistfences-review --config-dir <folder>`: the real Desktop with the review lockdown (see
/// `pecofence_platform::safety`): an imported configuration can be looked at before switching
/// from Fences, and nothing on the Desktop or in Windows can change.
const REVIEW_ARG: &str = "--mistfences-review";

/// `--mistfences-preview [--preview-root <folder>]`: the preview folder, or `None` for a normal
/// run.
fn preview_root() -> Option<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.iter().any(|a| a == preview::ARG) {
        return None;
    }
    args.iter()
        .position(|a| a == preview::ROOT_ARG)
        .and_then(|i| args.get(i + 1))
        .map(std::path::PathBuf::from)
        .or_else(preview::default_root)
        .or_else(|| Some(std::env::temp_dir().join("MistFences-Preview")))
}

/// `%LOCALAPPDATA%\PecoFence`: the logs (see `pecofence_core::brand::log_file_name`) and crash
/// dumps.
pub(crate) fn log_dir() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from)?;
    let dir = base.join(pecofence_core::brand::NAME);
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Starts this run's `current` log file, keeping the last run's as `previous` (a crash report
/// needs the log of the run that crashed). A second instance (PECOFENCE_INSTANCE) keeps its own
/// files.
fn start_log(
    dir: &std::path::Path,
    instance: Option<&str>,
    current: &str,
    previous: &str,
) -> Option<std::fs::File> {
    use pecofence_core::brand::log_file_name;
    let path = dir.join(log_file_name(instance, current));
    let prev = dir.join(log_file_name(instance, previous));
    // The Store edition installed on another drive cannot rename files in its copy of
    // %LOCALAPPDATA% (os error 17), and a viewer that holds the log without delete sharing
    // blocks the rename too: keep a copy instead.
    let kept = match std::fs::rename(&path, &prev) {
        Ok(()) => true,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(_) => std::fs::copy(&path, &prev).is_ok(),
    };
    if !kept {
        // Write on after the last run's lines rather than erase them.
        return std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .ok();
    }
    std::fs::File::create(path).ok()
}

/// `RUST_LOG` takes `level` and `target=level` directives, e.g. `pecofence=debug` (default
/// `info`). `Targets` instead of `EnvFilter` keeps the regex engine out of the binary; span and
/// field filters are not supported. Output goes to the log file and to stderr; the latter only
/// shows up when a console is attached. The shareable log (`share_log`) runs beside them.
fn init_logging(instance: Option<&str>) {
    use pecofence_core::brand;
    use tracing_subscriber::filter::Targets;
    use tracing_subscriber::fmt::writer::MakeWriterExt;
    use tracing_subscriber::prelude::*;
    let filter = std::env::var("RUST_LOG")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .and_then(|s| s.parse::<Targets>().ok())
        .unwrap_or_else(|| Targets::new().with_default(tracing::Level::INFO));
    let dir = log_dir();
    let file = dir
        .as_deref()
        .and_then(|d| start_log(d, instance, brand::LOG, brand::PREVIOUS_LOG));
    let share = dir
        .as_deref()
        .and_then(|d| start_log(d, instance, brand::SHARE_LOG, brand::PREVIOUS_SHARE_LOG))
        .map(share_log::ShareLayer::new);
    match file {
        Some(file) => {
            let file = std::sync::Mutex::new(file);
            tracing_subscriber::registry()
                .with(
                    tracing_subscriber::fmt::layer()
                        .with_ansi(false)
                        .with_writer(file.and(std::io::stderr)),
                )
                .with(share)
                .with(filter)
                .init();
        }
        None => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
                .with(share)
                .with(filter)
                .init();
        }
    }
}

/// Panics inside a window procedure or COM callback abort the process (GUI subsystem: no
/// console to print to), so the message and a backtrace go to the log first.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        // A literal panic message is code text; a formatted one may hold data, so it stays in
        // `detail`, which the shareable log omits.
        let payload = info
            .payload()
            .downcast_ref::<&'static str>()
            .copied()
            .unwrap_or("(formatted message)");
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!(%location, payload, detail = %info, backtrace = %bt, "PANIC");
    }));
}

/// Asks the running instance to open its settings window. Fire and forget: it answers late
/// while one of its menus is open, and this process has nothing else to do. Not when that
/// instance has only just started: two autostart entries (say the Store and the ZIP edition)
/// race at logon, and the loser must not open Settings.
fn show_running_settings(instance: Option<&str>) {
    use std::io::Write;
    use std::os::windows::fs::OpenOptionsExt;
    const SECURITY_IDENTIFICATION: u32 = 0x0001_0000;
    const STARTUP: std::time::Duration = std::time::Duration::from_secs(30);
    pecofence_platform::process::allow_foreground_handoff();
    let request = pecofence_ipc::Request::new(pecofence_ipc::Method::SettingsOpenUi);
    let Ok(name) = pecofence_platform::pipe::ipc_pipe_name(instance) else {
        return;
    };
    let pipe = std::fs::OpenOptions::new()
        .write(true)
        .security_qos_flags(SECURITY_IDENTIFICATION)
        .open(name);
    let Ok(mut pipe) = pipe else {
        return;
    };
    if pecofence_platform::process::pipe_server_age(&pipe).is_some_and(|age| age < STARTUP) {
        return;
    }
    if let Ok(line) = serde_json::to_string(&request) {
        let _ = writeln!(pipe, "{line}");
    }
}

fn main() -> Result<()> {
    // `--import-fences <snapshot.xml> --config-dir <folder>`: a run without windows that writes
    // a configuration (or only its report) and exits; see `fences_import`.
    let all_args: Vec<String> = std::env::args().skip(1).collect();
    if all_args.iter().any(|a| a == full_mode::DESK_REPORT_ARG) {
        let dir = all_args
            .iter()
            .position(|a| a == CONFIG_DIR_ARG)
            .and_then(|i| all_args.get(i + 1))
            .map(std::path::PathBuf::from);
        let Some(dir) = dir else {
            std::process::exit(1);
        };
        init_logging(Some("desk-report"));
        window::set_process_dpi_awareness_v2();
        let _ole = OleGuard::init()?;
        std::process::exit(full_mode::desk_report(&dir));
    }
    if all_args.iter().any(|a| a == full_mode::CHECK_ARG) {
        let found = pecofence_platform::fences_detect::detect();
        let lines = full_mode::fences_report(&found);
        if let Some(dir) = all_args
            .iter()
            .position(|a| a == CONFIG_DIR_ARG)
            .and_then(|i| all_args.get(i + 1))
        {
            let text = if lines.is_empty() {
                "Fences is not active
"
                .to_string()
            } else {
                format!(
                    "Fences is active:
{}
",
                    lines.join(
                        "
"
                    )
                )
            };
            let _ = std::fs::write(std::path::Path::new(dir).join("fences-check.txt"), text);
        }
        std::process::exit(if found.active() {
            full_mode::CHECK_ACTIVE
        } else {
            0
        });
    }
    if let Some(request) = fences_import::request(&all_args) {
        init_logging(Some("import"));
        install_panic_hook();
        let code = match request {
            Ok(request) => {
                // Work areas in physical pixels, as the fences will use them.
                window::set_process_dpi_awareness_v2();
                let _ole = OleGuard::init()?;
                fences_import::run(&request)
            }
            Err(error) => {
                tracing::error!(%error, "Fences import: bad command line");
                1
            }
        };
        std::process::exit(code);
    }
    // `PECOFENCE_INSTANCE=<name>` runs a second, independent instance (developer testing with
    // `--portable`); the default name keeps one PecoFence per session. Checked before logging
    // starts: a second launch must leave the running instance's log files alone (it used to
    // truncate the log on its way out).
    let preview_root = preview_root();
    let mut instance_name = pecofence_core::brand::var("PECOFENCE_INSTANCE").ok();
    // The preview and the review are always named instances (`preview`, or `preview-<name>` for
    // a parallel test), so their mutex, pipe and logs never meet a regular MistFences.
    let review = std::env::args().any(|a| a == REVIEW_ARG) && preview_root.is_none();
    if preview_root.is_some() || review {
        let base = if review {
            pecofence_core::brand::REVIEW_INSTANCE
        } else {
            pecofence_core::brand::PREVIEW_INSTANCE
        };
        instance_name = Some(match instance_name.as_deref().map(str::trim) {
            Some(n) if !n.is_empty() && n != base => format!("{base}-{n}"),
            _ => base.to_string(),
        });
    }
    let instance = instance_name
        .as_deref()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    let [current_name, legacy_name] =
        pecofence_core::brand::instance_mutex_names(instance_name.as_deref());
    let Some(_instance) = window::SingleInstance::acquire(&current_name) else {
        // Started again from the Start menu or a shortcut: Windows 11 hides new tray icons
        // in the overflow, so show the running copy's settings rather than exit silently.
        if std::env::args().len() == 1 {
            show_running_settings(instance.as_deref());
        }
        return Ok(());
    };
    // A pre-rename instance is running: exit it before starting PecoFence.
    let Some(_legacy_instance) = window::SingleInstance::acquire(&legacy_name) else {
        return Ok(());
    };

    init_logging(instance.as_deref());
    install_panic_hook();
    if let Some(dir) = log_dir() {
        pecofence_platform::crashlog::install(dir, instance.as_deref().unwrap_or("main"));
    }

    let mut args = parse_args();
    args.instance = instance;
    if let Some(root) = preview_root {
        // Before any window, COM object or watcher exists: the lockdown has to cover all of
        // them. A preview that cannot lock down does not run.
        let desktops: Vec<std::path::PathBuf> = [
            pecofence_platform::shell::user_desktop(),
            pecofence_platform::shell::public_desktop(),
        ]
        .into_iter()
        .flatten()
        .collect();
        let paths = match preview::prepare(&root, &desktops) {
            Ok(paths) => paths,
            Err(error) => {
                tracing::error!(root = %root.display(), %error, "preview folder unusable; exiting");
                return Ok(());
            }
        };
        if !pecofence_platform::safety::enable_preview(&paths.sandbox) {
            tracing::error!("preview lockdown could not be enabled; exiting");
            return Ok(());
        }
        tracing::info!(
            root = %paths.root.display(),
            sandbox = %paths.sandbox.display(),
            "MistFences preview"
        );
        // Explorer's icons stay as they are, whatever the configuration says.
        args.no_hide_icons = true;
        args.preview = Some(paths);
    } else if review {
        // The same before-anything-else rule as the preview. Without its own configuration
        // folder (outside the Desktop) the review does not run.
        let Some(dir) = args.config_dir.clone().filter(|d| d.is_absolute()) else {
            tracing::error!("--mistfences-review needs --config-dir <absolute folder>; exiting");
            return Ok(());
        };
        let inside_desktop = [
            pecofence_platform::shell::user_desktop(),
            pecofence_platform::shell::public_desktop(),
        ]
        .into_iter()
        .flatten()
        .any(|d| state::canonical_path(dir.clone()).starts_with(state::canonical_path(d)));
        if inside_desktop || std::fs::create_dir_all(&dir).is_err() {
            tracing::error!(dir = %dir.display(), "review folder unusable; exiting");
            return Ok(());
        }
        if !pecofence_platform::safety::enable_review(&dir) {
            tracing::error!("review lockdown could not be enabled; exiting");
            return Ok(());
        }
        tracing::info!(config = %dir.display(), "MistFences review");
        args.no_hide_icons = true;
        args.review = true;
    }
    // The full mode: never next to an active Fences, and safe after a crash loop.
    let journal = if args.preview.is_none() && !args.review {
        if !full_mode::fences_allow_start() {
            return Ok(());
        }
        let journal = full_mode::Journal::start(args.instance.as_deref());
        if journal.safe {
            args.no_hide_icons = true;
            args.safe_start = true;
        }
        Some(journal)
    } else {
        None
    };
    let exit_after = args.exit_after_ms;
    // Back after an update closed it. Not named test instances: their environment would be
    // lost and the restart would take the main instance's place.
    if args.instance.is_none()
        && exit_after.is_none()
        && let Err(e) = pecofence_platform::process::register_restart_after_update()
    {
        tracing::warn!(error = %e, "restart after updates not registered");
    }

    window::set_process_dpi_awareness_v2();
    let _ole = OleGuard::init()?;

    let cell = app::App::create(args)?;
    if let Some(ms) = exit_after {
        window::quit_after(ms);
    }

    let code = window::run_message_loop();
    if let Some(app) = cell.borrow_mut().as_mut() {
        app.shutdown();
    }
    drop(cell);
    if let Some(journal) = journal {
        journal.finish();
    }
    std::process::exit(code);
}
