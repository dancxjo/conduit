//! Read-only inventory: a target directory is not proof that its contents are disposable.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

#[derive(Default, Serialize)]
struct Usage {
    logical_bytes: u64,
    files: u64,
    skipped_symlinks: u64,
}

fn measure(path: &Path, usage: &mut Usage) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {}: {error}", path.display()))?;
    if metadata.is_symlink() {
        usage.skipped_symlinks += 1;
    } else if metadata.is_file() {
        usage.logical_bytes = usage
            .logical_bytes
            .checked_add(metadata.len())
            .ok_or("storage byte count overflow")?;
        usage.files += 1;
    } else if metadata.is_dir() {
        for entry in
            std::fs::read_dir(path).map_err(|error| format!("list {}: {error}", path.display()))?
        {
            let entry = entry.map_err(|error| format!("read directory entry: {error}"))?;
            measure(&entry.path(), usage)?;
        }
    } else {
        return Err(format!("unsupported filesystem object: {}", path.display()));
    }
    Ok(())
}

#[derive(Serialize)]
struct WorktreeUsage {
    worktree: String,
    target: String,
    exists: bool,
    usage: Usage,
}

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    for argument in arguments.iter().skip(2) {
        if argument != "--locked" {
            return Err(format!(
                "unsupported ci storage-report argument: {argument}"
            ));
        }
    }
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain", "-z"])
        .output()
        .map_err(|error| format!("list Git worktrees: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "list Git worktrees: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let listing = std::str::from_utf8(&output.stdout)
        .map_err(|_| "worktree paths must be UTF-8 for the JSON report")?;
    let mut worktrees = Vec::new();
    for worktree in listing
        .split('\0')
        .filter_map(|line| line.strip_prefix("worktree "))
    {
        let target = Path::new(worktree).join("target");
        let exists = match std::fs::symlink_metadata(&target) {
            Ok(_) => true,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(error) => return Err(format!("inspect {}: {error}", target.display())),
        };
        let mut usage = Usage::default();
        if exists {
            measure(&target, &mut usage)?;
        }
        worktrees.push(WorktreeUsage {
            worktree: worktree.into(),
            target: target.to_string_lossy().into_owned(),
            exists,
            usage,
        });
    }
    worktrees.sort_by(|a, b| a.worktree.cmp(&b.worktree));
    let report = serde_json::json!({
        "schema": "conduit.local-storage-report/v1",
        "measurement": "logical-file-bytes; hard links may be counted repeatedly",
        "scope": "registered Git worktree target directories only",
        "reclamation_authorized": false,
        "note": "Contents may include retained evidence or active output. This report does not classify anything as disposable. Symlinks are not followed. External Cargo target directories and caches are not inventoried.",
        "worktrees": worktrees,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_mutating_or_unknown_options_before_inventory() {
        let arguments = ["ci", "storage-report", "--delete"].map(String::from);
        assert!(run(&arguments).unwrap_err().contains("unsupported"));
    }

    #[test]
    fn measures_nested_files_and_reports_missing_paths() {
        let root = std::env::temp_dir().join(format!(
            "conduit-storage-measure-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("a"), b"abc").unwrap();
        std::fs::write(root.join("nested/b"), b"12345").unwrap();
        let mut usage = Usage::default();
        let result = measure(&root, &mut usage);
        std::fs::remove_dir_all(&root).unwrap();
        result.unwrap();
        assert_eq!(usage.logical_bytes, 8);
        assert_eq!(usage.files, 2);
        assert!(measure(&root, &mut Usage::default()).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn never_follows_symlinks() {
        let path =
            std::env::temp_dir().join(format!("conduit-storage-link-{}", std::process::id()));
        std::os::unix::fs::symlink("/", &path).unwrap();
        let mut usage = Usage::default();
        let result = measure(&path, &mut usage);
        std::fs::remove_file(&path).unwrap();
        result.unwrap();
        assert_eq!(usage.skipped_symlinks, 1);
        assert_eq!(usage.logical_bytes, 0);
        assert_eq!(usage.files, 0);
    }
}
