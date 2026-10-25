//! Focused Linux scanout proof, including real SDL software-texture readback.
use std::{path::Path, process::Command};

pub(super) fn run() -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("linux-display requires a Linux Host".into());
    }
    let root = crate::workspace::workspace_root()?;
    let output = Command::new("cc")
        .arg("-print-file-name=libSDL2-2.0.so.0")
        .output()
        .map_err(|e| format!("locate installed SDL2: {e}"))?;
    let runtime = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
    let runtime = Path::new(runtime.trim());
    if !output.status.success() || !runtime.is_absolute() || !runtime.is_file() {
        return Err(
            "SDL2 runtime unavailable; install the distribution's libsdl2-2.0-0 package".into(),
        );
    }
    let runtime = runtime.canonicalize().map_err(|e| e.to_string())?;
    let link_dir = root.join("target/linux-display-link");
    std::fs::create_dir_all(&link_dir).map_err(|e| e.to_string())?;
    let link = link_dir.join("libSDL2.so");
    if link.exists() {
        if link.canonicalize().map_err(|e| e.to_string())? != runtime {
            return Err("existing SDL2 linker alias selects a different runtime".into());
        }
    } else {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&runtime, &link).map_err(|e| e.to_string())?;
    }
    let mut paths = vec![link_dir];
    if let Some(existing) = std::env::var_os("LIBRARY_PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    let search = std::env::join_paths(paths).map_err(|e| e.to_string())?;
    for action in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(&root)
            .args([
                action,
                "--locked",
                "--manifest-path",
                "targets/linux/display/Cargo.toml",
                "--all-targets",
            ])
            .env("LIBRARY_PATH", &search);
        if std::env::var_os("CARGO_TARGET_DIR").is_none() {
            command.env("CARGO_TARGET_DIR", root.join("target/linux-display"));
        }
        if action == "clippy" {
            command.args(["--", "-D", "warnings"]);
        }
        let status = command.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Linux display {action} failed: {status}"));
        }
    }
    for action in ["test", "clippy"] {
        let mut command = Command::new("cargo");
        command
            .current_dir(&root)
            .args([
                action,
                "--locked",
                "-p",
                "conduit-std-host",
                "--lib",
                "--features",
                "linux-display",
            ])
            .env("LIBRARY_PATH", &search);
        if action == "test" {
            command.arg("hosted_display::");
        } else {
            command.args(["--tests", "--", "-D", "warnings"]);
        }
        let status = command.status().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("Linux display Host {action} failed: {status}"));
        }
    }
    Ok(())
}

/// Prepare only a bootstrap image; the Python owner refuses existing outputs,
/// block devices and a base image that differs from the pinned official digest.
pub(super) fn bootstrap(arguments: &[String]) -> Result<(), String> {
    let root = crate::workspace::workspace_root()?;
    let status = Command::new("python3")
        .current_dir(&root)
        .arg("targets/raspberry-pi/deployment/linux/bootstrap.py")
        .args(arguments)
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Pi bootstrap preparation failed: {status}"))
    }
}
