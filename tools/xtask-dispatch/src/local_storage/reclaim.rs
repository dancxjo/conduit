//! Conservative reclamation of inactive, regenerable Cargo compilation output.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_MAXIMUM_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const DAN_HOSTS: [&str; 3] = ["forebrain", "victus", "envie"];
const COMPILATION_DIRECTORIES: [&str; 5] =
    [".fingerprint", "build", "deps", "examples", "incremental"];

#[derive(Debug, Serialize)]
struct Candidate {
    path: String,
    logical_bytes: u64,
}

#[derive(Debug, Serialize)]
struct WorktreeDecision {
    worktree: String,
    disposition: &'static str,
    reason: &'static str,
    candidates: Vec<Candidate>,
}

#[derive(Debug)]
struct Options {
    apply: bool,
    maximum_bytes: u64,
}

pub(crate) fn run(arguments: &[String]) -> Result<(), String> {
    let options = options(arguments)?;
    let host = command_text("hostname", &[])?;
    if options.apply && !host_is_owned(host.trim()) {
        return Err(format!(
            "storage reclamation is refused on non-owned host {host:?}; use dry-run inventory"
        ));
    }
    if options.apply && !cfg!(target_os = "linux") {
        return Err("storage reclamation requires Linux process-ownership inspection".into());
    }

    let repository = command_text("git", &["rev-parse", "--show-toplevel"])?;
    let current = canonical(&PathBuf::from(repository.trim()))?;
    let mut remaining = options.maximum_bytes;
    let mut selected_bytes = 0_u64;
    let mut reclaimed_bytes = 0_u64;
    let mut decisions = Vec::new();

    for worktree in worktrees()? {
        let canonical_worktree = canonical(&worktree)?;
        let target = canonical_worktree.join("target");
        let mut decision = WorktreeDecision {
            worktree: canonical_worktree.to_string_lossy().into_owned(),
            disposition: "preserved",
            reason: "no compilation directories",
            candidates: Vec::new(),
        };
        if canonical_worktree == current {
            decision.reason = "current worktree";
        } else if !target.is_dir() {
            decision.reason = "target absent";
        } else if path_is_symlink(&target)? {
            decision.reason = "target is a symlink";
        } else if !worktree_clean(&canonical_worktree)? {
            decision.reason = "worktree has tracked or untracked changes";
        } else if !head_reachable_from_remote(&canonical_worktree)? {
            decision.reason = "HEAD is not reachable from a current remote ref";
        } else if target_is_active(&target)? {
            decision.reason = "live process references target";
        } else {
            decision.reason = "eligible inactive compilation output";
            for path in compilation_directories(&target)? {
                let bytes = logical_bytes(&path)?;
                if bytes > remaining {
                    continue;
                }
                remaining -= bytes;
                selected_bytes = selected_bytes
                    .checked_add(bytes)
                    .ok_or("selected byte count overflow")?;
                if options.apply {
                    std::fs::remove_dir_all(&path)
                        .map_err(|error| format!("remove {}: {error}", path.display()))?;
                    reclaimed_bytes = reclaimed_bytes
                        .checked_add(bytes)
                        .ok_or("reclaimed byte count overflow")?;
                }
                decision.candidates.push(Candidate {
                    path: path.to_string_lossy().into_owned(),
                    logical_bytes: bytes,
                });
            }
            if !decision.candidates.is_empty() {
                decision.disposition = if options.apply {
                    "reclaimed"
                } else {
                    "selected"
                };
            }
        }
        decisions.push(decision);
    }
    decisions.sort_by(|left, right| left.worktree.cmp(&right.worktree));
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "conduit.local-storage-reclamation/v1",
            "host": host.trim(),
            "mode": if options.apply { "apply" } else { "dry-run" },
            "maximum_bytes": options.maximum_bytes,
            "selected_logical_bytes": selected_bytes,
            "reclaimed_logical_bytes": reclaimed_bytes,
            "scope": "inactive Cargo compilation directories only; source, worktrees, root binaries, staged products, and proof evidence are preserved",
            "worktrees": decisions,
        }))
        .map_err(|error| error.to_string())?
    );
    Ok(())
}

fn options(arguments: &[String]) -> Result<Options, String> {
    let mut apply = false;
    let mut maximum_bytes = DEFAULT_MAXIMUM_BYTES;
    let mut values = arguments.iter().skip(2);
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--apply" => apply = true,
            "--maximum-bytes" => {
                maximum_bytes = values
                    .next()
                    .ok_or("missing --maximum-bytes value")?
                    .parse::<u64>()
                    .map_err(|_| "--maximum-bytes must be an unsigned integer")?;
            }
            other => return Err(format!("unsupported ci storage-reclaim argument: {other}")),
        }
    }
    if maximum_bytes == 0 {
        return Err("--maximum-bytes must be greater than zero".into());
    }
    Ok(Options {
        apply,
        maximum_bytes,
    })
}

fn host_is_owned(host: &str) -> bool {
    DAN_HOSTS.iter().any(|owned| {
        host == *owned
            || host
                .strip_prefix(owned)
                .is_some_and(|suffix| suffix.starts_with('.'))
    })
}

fn worktrees() -> Result<Vec<PathBuf>, String> {
    let output = command_bytes("git", &["worktree", "list", "--porcelain", "-z"], None)?;
    let listing = std::str::from_utf8(&output)
        .map_err(|_| "worktree paths must be UTF-8 for the JSON report")?;
    Ok(listing
        .split('\0')
        .filter_map(|line| line.strip_prefix("worktree "))
        .map(PathBuf::from)
        .collect())
}

fn worktree_clean(worktree: &Path) -> Result<bool, String> {
    Ok(command_bytes(
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
        Some(worktree),
    )?
    .is_empty())
}

fn head_reachable_from_remote(worktree: &Path) -> Result<bool, String> {
    Ok(!command_bytes(
        "git",
        &[
            "branch",
            "--remotes",
            "--contains",
            "HEAD",
            "--format=%(refname)",
        ],
        Some(worktree),
    )?
    .is_empty())
}

fn compilation_directories(target: &Path) -> Result<Vec<PathBuf>, String> {
    let mut profile_roots = vec![target.join("debug"), target.join("release")];
    for entry in read_dir_if_present(target)? {
        let path = entry.path();
        if path.is_dir() && !matches!(entry.file_name().to_str(), Some("debug" | "release")) {
            profile_roots.push(path.join("debug"));
            profile_roots.push(path.join("release"));
        }
    }
    let mut result = Vec::new();
    for profile in profile_roots {
        for name in COMPILATION_DIRECTORIES {
            let path = profile.join(name);
            if path.is_dir() && !path_is_symlink(&path)? {
                result.push(path);
            }
        }
    }
    result.sort();
    Ok(result)
}

fn logical_bytes(path: &Path) -> Result<u64, String> {
    if path_is_symlink(path)? {
        return Ok(0);
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    let mut total = 0_u64;
    for entry in read_dir_if_present(path)? {
        total = total
            .checked_add(logical_bytes(&entry.path())?)
            .ok_or("storage byte count overflow")?;
    }
    Ok(total)
}

fn target_is_active(target: &Path) -> Result<bool, String> {
    let proc = Path::new("/proc");
    if !proc.is_dir() {
        return Ok(false);
    }
    let target_text = target.to_string_lossy();
    #[cfg(unix)]
    use std::os::unix::fs::MetadataExt;
    #[cfg(unix)]
    let current_uid = std::fs::metadata(proc.join("self"))
        .map_err(|error| format!("inspect /proc/self: {error}"))?
        .uid();
    for process in read_dir_if_present(proc)? {
        if !process
            .file_name()
            .to_string_lossy()
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        {
            continue;
        }
        let root = process.path();
        #[cfg(unix)]
        if std::fs::metadata(&root)
            .map(|metadata| metadata.uid() != current_uid)
            .unwrap_or(true)
        {
            continue;
        }
        for link in [root.join("cwd"), root.join("exe")] {
            if std::fs::read_link(link).is_ok_and(|path| path.starts_with(target)) {
                return Ok(true);
            }
        }
        let descriptors = match std::fs::read_dir(root.join("fd")) {
            Ok(entries) => entries
                .map(|entry| entry.map_err(|error| format!("inspect process fd: {error}")))
                .collect::<Result<Vec<_>, _>>()?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("inspect {}: {error}", root.join("fd").display())),
        };
        for descriptor in descriptors {
            if std::fs::read_link(descriptor.path()).is_ok_and(|path| path.starts_with(target)) {
                return Ok(true);
            }
        }
        match std::fs::read_to_string(root.join("maps")) {
            Ok(maps) if maps.lines().any(|line| line.contains(target_text.as_ref())) => {
                return Ok(true)
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("inspect {}: {error}", root.join("maps").display())),
        }
    }
    Ok(false)
}

fn read_dir_if_present(path: &Path) -> Result<Vec<std::fs::DirEntry>, String> {
    match std::fs::read_dir(path) {
        Ok(entries) => entries
            .map(|entry| entry.map_err(|error| format!("read {}: {error}", path.display())))
            .collect(),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                || error.kind() == std::io::ErrorKind::PermissionDenied =>
        {
            Ok(Vec::new())
        }
        Err(error) => Err(format!("list {}: {error}", path.display())),
    }
}

fn path_is_symlink(path: &Path) -> Result<bool, String> {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.is_symlink())
        .map_err(|error| format!("inspect {}: {error}", path.display()))
}

fn canonical(path: &Path) -> Result<PathBuf, String> {
    path.canonicalize()
        .map_err(|error| format!("resolve {}: {error}", path.display()))
}

fn command_text(program: &str, arguments: &[&str]) -> Result<String, String> {
    String::from_utf8(command_bytes(program, arguments, None)?)
        .map_err(|_| format!("{program} output was not UTF-8"))
}

fn command_bytes(program: &str, arguments: &[&str], cwd: Option<&Path>) -> Result<Vec<u8>, String> {
    let mut command = Command::new(program);
    command.args(arguments);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|error| format!("run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_requires_explicit_apply_and_a_positive_bound() {
        let dry = ["ci", "storage-reclaim", "--maximum-bytes", "9"].map(String::from);
        let parsed = options(&dry).unwrap();
        assert!(!parsed.apply);
        assert_eq!(parsed.maximum_bytes, 9);

        let zero = ["ci", "storage-reclaim", "--apply", "--maximum-bytes", "0"].map(String::from);
        assert!(options(&zero).is_err());
    }

    #[test]
    fn only_explicit_owned_hostnames_admit_apply_mode() {
        assert!(host_is_owned("forebrain"));
        assert!(host_is_owned("forebrain.local"));
        assert!(!host_is_owned("forebrain-attacker"));
        assert!(!host_is_owned("runner"));
    }

    #[test]
    fn compilation_scope_does_not_select_root_products_or_evidence() {
        let root = std::env::temp_dir().join(format!("conduit-reclaim-{}", std::process::id()));
        std::fs::create_dir_all(root.join("debug/deps")).unwrap();
        std::fs::create_dir_all(root.join("browser-product/evidence")).unwrap();
        std::fs::write(root.join("debug/deps/library"), b"build").unwrap();
        std::fs::write(root.join("browser-product/evidence/receipt.json"), b"proof").unwrap();
        std::fs::write(root.join("debug/conduit"), b"product").unwrap();

        let selected = compilation_directories(&root).unwrap();
        assert_eq!(selected, vec![root.join("debug/deps")]);
        assert!(!selected.iter().any(|path| path.ends_with("evidence")));
        assert!(!selected.iter().any(|path| path.ends_with("conduit")));
        std::fs::remove_dir_all(root).unwrap();
    }
}
