//! Product identity. MistFences is a modification of PecoFence (Apache-2.0); everything that
//! names a shared Windows resource (mutexes, pipes, data folders, the Run entry, environment
//! variables) carries the MistFences name so it never touches a PecoFence installation.

use std::env::VarError;
use std::ffi::OsString;

pub const NAME: &str = "MistFences";
/// The project MistFences is based on (attribution, About page).
pub const UPSTREAM_NAME: &str = "PecoFence";
/// Microsoft Store product id. MistFences has no Store listing: empty.
pub const STORE_PRODUCT_ID: &str = "";
/// MistFences' own repository (About page, manual, theme guide).
pub const REPOSITORY_URL: &str = "https://github.com/Mist10X/MistFences";
/// The project it is based on.
pub const UPSTREAM_REPOSITORY_URL: &str = "https://github.com/DayuanJiang/PecoFence";
/// Pre-rename data folder adopted on first start. MistFences has none: a MistFences-only name
/// that no installation creates, so it never reads PecoFence's or OpenFence's data.
pub const LEGACY_DATA_DIR: &str = "MistFences.Legacy";
/// Pre-rename Run value removed when autostart is reconciled. A MistFences-only name, so the
/// reconciliation never deletes another program's startup entry.
pub const LEGACY_AUTOSTART: &str = "MistFences.Legacy";
/// Upstream's network services (feedback upload, update download, Store rating) belong to
/// PecoFence: MistFences talks to none of them.
pub const REMOTE_SERVICES: bool = false;
// A change here must be deliberate: it would send data to PecoFence.s servers.
const _: () = assert!(!REMOTE_SERVICES);
/// Instance name of the safe preview (`--mistfences-preview`): its own mutex, pipe and logs.
pub const PREVIEW_INSTANCE: &str = "preview";
/// Instance name of `--mistfences-review` (the real Desktop, read only).
pub const REVIEW_INSTANCE: &str = "review";

/// Environment variables keep their upstream names in the code (`PECOFENCE_INSTANCE`, ...)
/// but are read as `MISTFENCES_*` only, so a PecoFence setup on the same machine cannot steer
/// MistFences (and the other way round).
fn lookup_env(name: &str, lookup: impl Fn(&str) -> Option<OsString>) -> Option<OsString> {
    match name.strip_prefix("PECOFENCE_") {
        Some(suffix) => lookup(&format!("MISTFENCES_{suffix}")),
        None => lookup(name),
    }
}

/// The value of an environment variable, under its MistFences name.
pub fn var_os(name: &str) -> Option<OsString> {
    lookup_env(name, |key| std::env::var_os(key))
}

pub fn var(name: &str) -> Result<String, VarError> {
    var_os(name)
        .ok_or(VarError::NotPresent)?
        .into_string()
        .map_err(VarError::NotUnicode)
}

/// Single-instance mutexes. Named test instances remain independent of the main app. The
/// second name only keeps the shape upstream uses for its pre-rename guard.
pub fn instance_mutex_names(instance: Option<&str>) -> [String; 2] {
    let suffix = instance_suffix(instance);
    [
        format!(r"Local\MistFences.SingleInstance{suffix}"),
        format!(r"Local\MistFences.SingleInstance.Compat{suffix}"),
    ]
}

/// Named pipe the CLI (`mistfences-cli`) talks to: `MistFences.<user SID>[.<instance>]`. Pipe
/// names are machine-wide, so the SID keeps two signed-in accounts apart, while the same user
/// in another session (ssh) still reaches their own instance.
pub fn ipc_pipe_name(user_sid: &str, instance: Option<&str>) -> String {
    format!(r"\\.\pipe\{NAME}.{user_sid}{}", instance_suffix(instance))
}

/// [`log_file_name`] suffixes: this run's log, the previous run's, and the shareable log (only
/// what may be sent with feedback) of this and the previous run.
pub const LOG: &str = "";
pub const PREVIOUS_LOG: &str = ".prev";
pub const SHARE_LOG: &str = ".share";
pub const PREVIOUS_SHARE_LOG: &str = ".share.prev";

/// Log file in `%LOCALAPPDATA%\MistFences`: `mistfences[.name]<suffix>.log`, one set per
/// instance.
pub fn log_file_name(instance: Option<&str>, suffix: &str) -> String {
    format!("mistfences{}{suffix}.log", instance_suffix(instance))
}

/// `.name` for a named (test) instance, empty for the main one; whitespace-only = main.
fn instance_suffix(instance: Option<&str>) -> String {
    instance
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!(".{value}"))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_is_read_under_the_mistfences_name_only() {
        let all = |name: &str| match name {
            "MISTFENCES_INSTANCE" => Some(OsString::from("mist")),
            "PECOFENCE_INSTANCE" => Some(OsString::from("peco")),
            "OPENFENCE_INSTANCE" => Some(OsString::from("open")),
            _ => None,
        };
        assert_eq!(lookup_env("PECOFENCE_INSTANCE", all), Some("mist".into()));
        // A PecoFence or OpenFence variable alone does not reach MistFences.
        assert_eq!(
            lookup_env("PECOFENCE_INSTANCE", |name| {
                matches!(name, "PECOFENCE_INSTANCE" | "OPENFENCE_INSTANCE").then(|| "x".into())
            }),
            None
        );
        assert_eq!(
            lookup_env("PECOFENCE_INSTANCE", |name| {
                (name == "MISTFENCES_INSTANCE").then(OsString::new)
            }),
            Some(OsString::new())
        );
        assert_eq!(lookup_env("UNRELATED", all), None);
    }

    #[test]
    fn instance_names_are_mistfences_and_use_the_normalized_suffix() {
        let main = instance_mutex_names(None);
        assert_eq!(main[0], r"Local\MistFences.SingleInstance");
        assert_eq!(main, instance_mutex_names(Some("  ")));
        let named = instance_mutex_names(Some(" test "));
        assert_eq!(named[0], r"Local\MistFences.SingleInstance.test");
        assert_eq!(named[1], r"Local\MistFences.SingleInstance.Compat.test");
        assert_ne!(main, named);
        for name in main.iter().chain(&named) {
            assert!(
                !name.contains("PecoFence") && !name.contains("openFence"),
                "{name}"
            );
        }
    }

    #[test]
    fn pipe_name_carries_the_user_sid_and_the_instance_suffix() {
        let sid = "S-1-5-21-1-2-3-1001";
        assert_eq!(
            ipc_pipe_name(sid, None),
            r"\\.\pipe\MistFences.S-1-5-21-1-2-3-1001"
        );
        assert_eq!(ipc_pipe_name(sid, Some(" ")), ipc_pipe_name(sid, None));
        assert_eq!(
            ipc_pipe_name(sid, Some(" test ")),
            r"\\.\pipe\MistFences.S-1-5-21-1-2-3-1001.test"
        );
        assert_ne!(
            ipc_pipe_name(sid, None),
            ipc_pipe_name("S-1-5-21-1-2-3-1002", None)
        );
    }

    #[test]
    fn log_names_follow_the_instance_suffix() {
        assert_eq!(log_file_name(None, LOG), "mistfences.log");
        assert_eq!(
            log_file_name(Some(" "), PREVIOUS_LOG),
            "mistfences.prev.log"
        );
        assert_eq!(
            log_file_name(Some("test"), SHARE_LOG),
            "mistfences.test.share.log"
        );
        assert_eq!(
            log_file_name(Some("test"), PREVIOUS_SHARE_LOG),
            "mistfences.test.share.prev.log"
        );
    }

    #[test]
    fn no_shared_name_points_at_another_product() {
        assert!(LEGACY_DATA_DIR.starts_with(NAME) && LEGACY_DATA_DIR != NAME);
        assert!(LEGACY_AUTOSTART.starts_with(NAME));
        assert!(STORE_PRODUCT_ID.is_empty());
    }
}
