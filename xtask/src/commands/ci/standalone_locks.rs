use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StandaloneLock {
    manifest: &'static str,
    lock: &'static str,
}

const LOCKS: &[StandaloneLock] = &[
    StandaloneLock {
        manifest: "targets/esp32/firmware/c3-signal/Cargo.toml",
        lock: "targets/esp32/firmware/c3-signal/Cargo.lock",
    },
    StandaloneLock {
        manifest: "targets/esp32/firmware/s3-signal/Cargo.toml",
        lock: "targets/esp32/firmware/s3-signal/Cargo.lock",
    },
    StandaloneLock {
        manifest: "targets/esp32/firmware/wroom-signal/Cargo.toml",
        lock: "targets/esp32/firmware/wroom-signal/Cargo.lock",
    },
    StandaloneLock {
        manifest: "targets/esp32/firmware/wroom-signal/fabrication-package-runner/Cargo.toml",
        lock: "targets/esp32/firmware/wroom-signal/fabrication-package-runner/Cargo.lock",
    },
    StandaloneLock {
        manifest: "targets/rp2040/firmware/pico-w-signal/Cargo.toml",
        lock: "targets/rp2040/firmware/pico-w-signal/Cargo.lock",
    },
];

pub(super) fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = crate::workspace::workspace_root()?;
    let failures = check_all(&root, check_with_cargo);
    if failures.is_empty() {
        println!("standalone_locks_checked={}", LOCKS.len());
        return Ok(());
    }

    Err(format!(
        "standalone Cargo lock preflight failed:\n{}\nregenerate each named lock deliberately before fabrication",
        failures
            .iter()
            .map(|failure| format!("- {failure}"))
            .collect::<Vec<_>>()
            .join("\n")
    )
    .into())
}

fn check_all(
    root: &Path,
    mut check: impl FnMut(&Path, StandaloneLock) -> Result<(), String>,
) -> Vec<String> {
    LOCKS
        .iter()
        .filter_map(|lock| check(root, *lock).err())
        .collect()
}

fn check_with_cargo(root: &Path, lock: StandaloneLock) -> Result<(), String> {
    for path in [lock.manifest, lock.lock] {
        if !root.join(path).is_file() {
            return Err(format!("{}: missing {path}", lock.manifest));
        }
    }

    // Resolve the graph without downloading crate sources or upgrading existing
    // registry packages. --locked refuses any required lock change.
    let output = Command::new(cargo_program())
        .args([
            "update",
            "--locked",
            "--workspace",
            "--manifest-path",
            lock.manifest,
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("{}: cannot launch Cargo: {error}", lock.manifest))?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let detail = stderr
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("Cargo rejected the locked dependency graph");
    Err(format!("{}: {} ({detail})", lock.manifest, lock.lock))
}

fn cargo_program() -> PathBuf {
    std::env::var_os("CARGO")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("cargo"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn registry_covers_every_separately_rooted_checked_in_lock() {
        let root = crate::workspace::workspace_root().unwrap();
        let mut discovered = Vec::new();
        discover_locks(&root, &root, &mut discovered);
        let registered: BTreeSet<_> = LOCKS.iter().map(|lock| lock.lock.to_owned()).collect();
        let discovered: BTreeSet<_> = discovered.into_iter().collect();
        assert_eq!(registered, discovered);
    }

    #[test]
    fn preflight_reports_every_failure_in_one_pass() {
        let root = Path::new("/unused");
        let failures = check_all(root, |_root, lock| {
            if lock.manifest.contains("esp32") {
                Err(format!("{} is stale", lock.lock))
            } else {
                Ok(())
            }
        });
        assert_eq!(failures.len(), 4);
        assert!(failures.iter().any(|failure| failure.contains("c3-signal")));
        assert!(failures.iter().any(|failure| failure.contains("s3-signal")));
        assert!(failures
            .iter()
            .any(|failure| failure.contains("wroom-signal")));
    }

    #[test]
    fn changed_path_dependency_is_refused_when_the_lock_is_stale() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "conduit-standalone-lock-test-{}-{nonce}",
            std::process::id()
        ));
        for name in ["app", "dependency", "added"] {
            std::fs::create_dir_all(root.join(name).join("src")).unwrap();
            std::fs::write(root.join(name).join("src/lib.rs"), "pub fn present() {}\n").unwrap();
            std::fs::write(
                root.join(name).join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n"),
            )
            .unwrap();
        }
        let append_dependency = |name: &str, dependency: &str| {
            let manifest = root.join(name).join("Cargo.toml");
            let mut source = std::fs::read_to_string(&manifest).unwrap();
            source.push_str(&format!(
                "\n[dependencies]\n{dependency} = {{ path = \"../{dependency}\" }}\n"
            ));
            std::fs::write(manifest, source).unwrap();
        };
        append_dependency("app", "dependency");
        let generate_lock = || {
            assert!(Command::new(cargo_program())
                .args([
                    "generate-lockfile",
                    "--offline",
                    "--manifest-path",
                    "app/Cargo.toml"
                ])
                .current_dir(&root)
                .status()
                .unwrap()
                .success());
        };
        let lock = StandaloneLock {
            manifest: "app/Cargo.toml",
            lock: "app/Cargo.lock",
        };
        generate_lock();
        check_with_cargo(&root, lock).unwrap();
        let original_lock = std::fs::read(root.join(lock.lock)).unwrap();

        append_dependency("dependency", "added");
        let error = check_with_cargo(&root, lock).unwrap_err();
        assert!(error.contains("app/Cargo.lock"));
        assert_eq!(std::fs::read(root.join(lock.lock)).unwrap(), original_lock);

        generate_lock();
        check_with_cargo(&root, lock).unwrap();

        std::fs::remove_dir_all(root).unwrap();
    }

    fn discover_locks(root: &Path, directory: &Path, found: &mut Vec<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                if matches!(entry.file_name().to_str(), Some(".git" | "target")) {
                    continue;
                }
                discover_locks(root, &path, found);
            } else if entry.file_name() == "Cargo.lock" && path != root.join("Cargo.lock") {
                found.push(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
}
