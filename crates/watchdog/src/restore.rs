//! `mistfences-watchdog --restore [--no-autostart] [--quiet]`: the emergency way back to the
//! plain Windows desktop, for when MistFences misbehaves. Run it from the MistFences folder, a
//! shortcut or Win+R:
//!
//! 1. asks a running MistFences to quit, as its tray menu's Quit does, and waits for it;
//! 2. shows the Windows desktop icons again if they are hidden;
//! 3. removes the crash-recovery marker, so no later start takes the icons for its own;
//! 4. with `--no-autostart`, removes MistFences' startup entry.
//!
//! It never touches Fences, files on the Desktop or anything else, and says what it did.

use pecofence_platform::{autostart, desktop, shell_icons, window};
use std::io::Write;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub fn run(args: &[String]) -> i32 {
    let quiet = args.iter().any(|a| a == "--quiet");
    let no_autostart = args.iter().any(|a| a == "--no-autostart");
    let mut report = Vec::new();
    let mut ok = true;

    // 1. A running MistFences quits first: otherwise it would hide the icons again.
    let [mutex, _] = pecofence_core::brand::instance_mutex_names(None);
    if window::SingleInstance::is_held(&mutex) {
        ask_to_quit();
        let deadline = Instant::now() + Duration::from_secs(15);
        while window::SingleInstance::is_held(&mutex) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(200));
        }
        if window::SingleInstance::is_held(&mutex) {
            ok = false;
            report.push(
                "MistFences не закрылся за 15 секунд: закройте его в Диспетчере задач (mistfences.exe) и запустите восстановление ещё раз.".to_string(),
            );
        } else {
            report.push("MistFences закрыт.".to_string());
        }
    } else {
        report.push("MistFences не запущен.".to_string());
    }

    // 2. The icons.
    if shell_icons::desktop_icons_hidden() {
        let shown = (0..10).any(|attempt| {
            if attempt > 0 {
                std::thread::sleep(Duration::from_secs(1));
            }
            desktop::resolve_icon_host(desktop::detect_generation()).is_some_and(|host| {
                shell_icons::set_desktop_icons_hidden(false, host.def_view, host.host)
                    == Some(false)
            })
        });
        if shown {
            report.push("Значки рабочего стола Windows снова показаны.".to_string());
        } else {
            ok = false;
            report.push(
                "Значки не удалось показать: щёлкните правой кнопкой по рабочему столу → Вид → «Отображать значки рабочего стола».".to_string(),
            );
        }
    } else {
        report.push("Значки рабочего стола Windows уже видны.".to_string());
    }

    // 3. Markers (only once the icons show: a marker left behind still lets the next start
    //    restore them).
    if !shell_icons::desktop_icons_hidden() {
        for marker in markers() {
            if marker.exists() && std::fs::remove_file(&marker).is_ok() {
                report.push(format!("Удалён маркер {}.", marker.display()));
            }
        }
    }

    // 4. Startup entry.
    if no_autostart {
        match autostart::set_product_enabled(false, false) {
            Ok(()) => report.push("Автозапуск MistFences выключен.".to_string()),
            Err(e) => {
                ok = false;
                report.push(format!("Автозапуск выключить не удалось: {e}"));
            }
        }
    }

    let text = report.join("\n");
    let _ = std::io::stdout().write_all(format!("{text}\n").as_bytes());
    if !quiet {
        window::alert(&text, "MistFences — восстановление рабочего стола");
    }
    if ok { 0 } else { 1 }
}

/// The main instance's markers (the current and the pre-rename folder).
fn markers() -> Vec<PathBuf> {
    let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
        return Vec::new();
    };
    [
        pecofence_core::brand::NAME,
        pecofence_core::brand::LEGACY_DATA_DIR,
    ]
    .iter()
    .map(|dir| base.join(dir).join("icons-hidden.marker"))
    .collect()
}

/// Sends `app.quit` over the main instance's pipe; fire and forget.
fn ask_to_quit() {
    let Ok(name) = pecofence_platform::pipe::ipc_pipe_name(None) else {
        return;
    };
    let Ok(mut pipe) = std::fs::OpenOptions::new().write(true).open(name) else {
        return;
    };
    let request = pecofence_ipc::Request::new(pecofence_ipc::Method::AppQuit);
    if let Ok(line) = serde_json::to_string(&request) {
        let _ = writeln!(pipe, "{line}");
    }
}
