//! Lockdown (MistFences). Two modes keep a run from changing anything shared with Windows,
//! Explorer or other programs: desktop icon visibility, startup entries, Explorer's windows,
//! global input, the shell's context menus. The checks sit in the platform functions that
//! would make such a change, so every caller is covered: menus, hotkeys, drag and drop, the CLI
//! over IPC, Settings, test scripts.
//!
//! - `--mistfences-preview`: a sandbox folder stands in for the Desktop; files may change only
//!   inside it.
//! - `--mistfences-review`: the real Desktop is shown (to check an import before switching
//!   from Fences), and no file changes at all: items can be opened, nothing else.
//!
//! The lockdown is switched on once, before any window exists, and cannot be switched off.

use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

enum Lockdown {
    /// The sandbox folder.
    Preview(PathBuf),
    /// The folder the review's own data (configuration, exports) lives in.
    Review(PathBuf),
}

static LOCKDOWN: OnceLock<Lockdown> = OnceLock::new();

/// Turns the preview lockdown on for the rest of the process. `sandbox` is the folder that
/// stands in for the Desktop; it must exist. Returns false (and changes nothing) if a lockdown
/// is already on with another folder or mode, or the folder cannot be resolved.
pub fn enable_preview(sandbox: &Path) -> bool {
    let Some(root) = normalize(sandbox) else {
        return false;
    };
    let set = LOCKDOWN.get_or_init(|| Lockdown::Preview(root.clone()));
    let ok = matches!(set, Lockdown::Preview(r) if *r == root);
    if ok {
        tracing::info!(target: "mistfences::safety", sandbox = %root.display(), "preview lockdown on");
    }
    ok
}

/// Turns the review lockdown on: the real Desktop, read only. `data` is the folder its own
/// files (configuration, exports) may be written to; it must exist.
pub fn enable_review(data: &Path) -> bool {
    let Some(root) = normalize(data) else {
        return false;
    };
    let set = LOCKDOWN.get_or_init(|| Lockdown::Review(root.clone()));
    let ok = matches!(set, Lockdown::Review(r) if *r == root);
    if ok {
        tracing::info!(target: "mistfences::safety", data = %root.display(), "review lockdown on");
    }
    ok
}

/// Whether the sandboxed preview is on (its Desktop is the sandbox folder).
pub fn preview_active() -> bool {
    matches!(LOCKDOWN.get(), Some(Lockdown::Preview(_)))
}

/// Whether the read-only review of the real Desktop is on.
pub fn review_active() -> bool {
    matches!(LOCKDOWN.get(), Some(Lockdown::Review(_)))
}

/// Whether either lockdown is on.
pub fn lockdown_active() -> bool {
    LOCKDOWN.get().is_some()
}

/// The lockdown's name for files and messages: `preview` or `review`.
pub fn lockdown_name() -> Option<&'static str> {
    LOCKDOWN.get().map(|l| match l {
        Lockdown::Preview(_) => "preview",
        Lockdown::Review(_) => "review",
    })
}

/// The sandbox folder while the preview is on.
pub fn sandbox() -> Option<&'static Path> {
    match LOCKDOWN.get() {
        Some(Lockdown::Preview(s)) => Some(s),
        _ => None,
    }
}

/// A change to Windows-wide state (`op` names it in the log). Always allowed without a
/// lockdown; refused and logged under one.
pub fn allow_system_change(op: &str) -> bool {
    let Some(mode) = lockdown_name() else {
        return true;
    };
    tracing::warn!(target: "mistfences::safety", op, mode, "refused: system change");
    false
}

/// A change to the files at `paths` (sources and destinations alike). Without a lockdown always
/// allowed; in the preview only when every path resolves into the sandbox; in the review never.
pub fn allow_file_change(op: &str, paths: &[&Path]) -> bool {
    let lockdown = LOCKDOWN.get();
    if files_may_change(lockdown, paths) {
        return true;
    }
    let path = paths
        .first()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    tracing::warn!(
        target: "mistfences::safety",
        op,
        mode = lockdown_name().unwrap_or_default(),
        path,
        "refused: file change"
    );
    false
}

/// What [`allow_file_change`] would answer, without logging: for plans and dry runs that
/// must tell the caller up front which changes will be refused.
pub fn file_change_allowed(paths: &[&Path]) -> bool {
    files_may_change(LOCKDOWN.get(), paths)
}

fn files_may_change(lockdown: Option<&Lockdown>, paths: &[&Path]) -> bool {
    match lockdown {
        None => true,
        Some(Lockdown::Preview(root)) => paths.iter().all(|p| is_inside(root, p)),
        Some(Lockdown::Review(_)) => false,
    }
}

/// Writing one of the lockdown's own data files (a configuration export) to `path`: only into
/// its folder (the preview's: the sandbox's parent; the review's data folder), so an export can
/// never land on, or overwrite a file of, the real Desktop. Always allowed without a lockdown.
pub fn allow_preview_data_write(op: &str, path: &Path) -> bool {
    if data_may_be_written(LOCKDOWN.get(), path) {
        return true;
    }
    tracing::warn!(
        target: "mistfences::safety",
        op,
        path = %path.display(),
        "refused: path outside the lockdown's folder"
    );
    false
}

fn data_may_be_written(lockdown: Option<&Lockdown>, path: &Path) -> bool {
    let root = match lockdown {
        None => return true,
        Some(Lockdown::Preview(s)) => s.parent(),
        Some(Lockdown::Review(d)) => Some(d.as_path()),
    };
    root.is_some_and(|root| is_inside(root, path))
}

/// The error a refused operation returns.
pub fn refused(op: &str) -> windows_core::Error {
    windows_core::Error::new(
        windows_core::HRESULT(0x8007_0005u32 as i32), // E_ACCESSDENIED
        format!(
            "{op}: not allowed in the MistFences {}",
            lockdown_name().unwrap_or("lockdown")
        ),
    )
}

/// The same refusal for `std::io` callers.
pub fn refused_io(op: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!(
            "{op}: not allowed in the MistFences {}",
            lockdown_name().unwrap_or("lockdown")
        ),
    )
}

/// Whether `path` lies inside `root` (both resolved the way the file system will see them:
/// links and junctions followed, `..` removed, case ignored).
fn is_inside(root: &Path, path: &Path) -> bool {
    let Some(path) = normalize(path) else {
        return false;
    };
    let root = root.to_string_lossy().to_lowercase();
    let path = path.to_string_lossy().to_lowercase();
    let root = root.trim_end_matches('\\');
    path == root || path.starts_with(&format!("{root}\\"))
}

/// `path` resolved for comparison: the canonical form of its deepest existing ancestor (links
/// followed) with the rest appended, without `\\?\`. `None` for a relative path, or when a
/// remaining part would climb out (`..`).
fn normalize(path: &Path) -> Option<PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    let real = loop {
        if let Ok(real) = std::fs::canonicalize(&existing) {
            break real;
        }
        let name = existing.file_name()?.to_os_string();
        rest.push(name);
        existing = existing.parent()?.to_path_buf();
    };
    let text = real.to_string_lossy();
    let mut out = match text.strip_prefix(r"\\?\UNC\") {
        Some(unc) => PathBuf::from(format!(r"\\{unc}")),
        None => PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text)),
    };
    for part in rest.iter().rev() {
        // `file_name` never yields `..`, but a component like `a..` is a name; refuse anything
        // that is not a normal component to be safe.
        match Path::new(part).components().next() {
            Some(Component::Normal(_)) => out.push(part),
            _ => return None,
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mistfences-safety-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn inside_means_below_the_root_after_resolving() {
        let root = scratch("inside");
        let sandbox = root.join("Sandbox");
        std::fs::create_dir_all(sandbox.join("sub")).unwrap();
        let resolved = normalize(&sandbox).unwrap();
        assert!(is_inside(&resolved, &sandbox.join("a.txt")));
        assert!(is_inside(
            &resolved,
            &sandbox.join("sub").join("new folder")
        ));
        assert!(is_inside(&resolved, &sandbox));
        // Case does not matter on NTFS.
        let upper = PathBuf::from(sandbox.to_string_lossy().to_uppercase()).join("A.TXT");
        assert!(is_inside(&resolved, &upper));
        // A sibling whose name only starts like the sandbox is outside.
        assert!(!is_inside(&resolved, &root.join("Sandbox2").join("a.txt")));
        assert!(!is_inside(&resolved, &root.join("a.txt")));
        // Climbing out through `..` is resolved first.
        assert!(!is_inside(
            &resolved,
            &sandbox.join("sub").join("..").join("..").join("escape.txt")
        ));
        assert!(!is_inside(&resolved, Path::new("relative.txt")));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_review_changes_no_file_and_writes_only_its_own_data() {
        let root = scratch("review");
        let data = normalize(&root).unwrap();
        let review = Lockdown::Review(data.clone());
        let inside = data.join("config.json");
        let desktop = PathBuf::from(r"C:\Users\Public\Desktop\x.lnk");
        // No file may change, not even inside its own folder (only its data files are written).
        assert!(!files_may_change(Some(&review), &[&inside]));
        assert!(!files_may_change(Some(&review), &[&desktop]));
        assert!(data_may_be_written(Some(&review), &inside));
        assert!(!data_may_be_written(Some(&review), &desktop));
        // Without a lockdown everything goes; the preview keeps to its sandbox.
        assert!(files_may_change(None, &[&desktop]));
        let preview = Lockdown::Preview(data.clone());
        assert!(files_may_change(Some(&preview), &[&inside]));
        assert!(!files_may_change(Some(&preview), &[&desktop]));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn the_real_desktop_is_outside_any_sandbox() {
        let root = scratch("desktop");
        let resolved = normalize(&root).unwrap();
        if let Some(home) = std::env::var_os("USERPROFILE") {
            let desktop = PathBuf::from(home).join("Desktop").join("x.lnk");
            assert!(!is_inside(&resolved, &desktop));
        }
        assert!(!is_inside(
            &resolved,
            Path::new(r"C:\Users\Public\Desktop\x.lnk")
        ));
        let _ = std::fs::remove_dir_all(root);
    }
}
