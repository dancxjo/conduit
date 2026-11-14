//! Observe the exact build-script output selected by this Cargo invocation.
//! Cached build events are accepted; arbitrary target-directory searches are not.
use serde::Deserialize;
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
const MAXIMUM_EVENT_BYTES: u64 = 1024 * 1024;
#[derive(Deserialize)]
struct Event<'a> {
    reason: &'a str,
    package_id: Option<&'a str>,
    out_dir: Option<&'a str>,
}
fn event_out_dir(
    line: &[u8],
    package: &str,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let event: Event<'_> = serde_json::from_slice(line)?;
    if event.reason != "build-script-executed" {
        return Ok(None);
    }
    let Some(id) = event.package_id else {
        return Err("Cargo build-script event has no package identity".into());
    };
    let selected = id
        .rsplit_once('#')
        .and_then(|(_, tail)| tail.split_once('@'))
        .is_some_and(|(name, version)| name == package && !version.is_empty());
    if !selected {
        return Ok(None);
    }
    let path = PathBuf::from(
        event
            .out_dir
            .ok_or("selected Cargo build-script event has no output directory")?,
    );
    if !path.is_absolute() {
        return Err("selected Cargo output directory is not absolute".into());
    }
    Ok(Some(path))
}
fn checked_out_dir(path: &Path, target: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = path.canonicalize()?;
    let target = target.canonicalize()?;
    if path == target || !path.starts_with(&target) {
        return Err("Cargo build-script output escaped checked target directory".into());
    }
    Ok(path)
}
pub(super) fn run(
    command: &mut Command,
    target: &Path,
    package: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    command
        .arg("--message-format=json-render-diagnostics")
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    let mut child = command.spawn()?;
    let result: Result<PathBuf, Box<dyn std::error::Error>> = (|| {
        let stdout = child.stdout.take().ok_or("Cargo stdout custody absent")?;
        let mut reader = BufReader::new(stdout);
        let mut line = Vec::new();
        let mut selected = None;
        loop {
            line.clear();
            let count =
                Read::take(&mut reader, MAXIMUM_EVENT_BYTES + 1).read_until(b'\n', &mut line)?;
            if count == 0 {
                break;
            }
            if count as u64 > MAXIMUM_EVENT_BYTES {
                return Err("Cargo event exceeds finite observation bound".into());
            }
            if let Some(path) = event_out_dir(&line, package)? {
                let path = checked_out_dir(&path, target)?;
                if selected.as_ref().is_some_and(|prior| prior != &path) {
                    return Err("ambiguous selected Cargo build-script outputs".into());
                }
                selected = Some(path);
            }
        }
        selected.ok_or_else(|| "Cargo did not report the selected Host build-script output".into())
    })();
    if result.is_err() {
        let _ = child.kill();
    }
    let status = child.wait()?;
    match result {
        Err(error) => Err(format!("native Cargo observation refused ({status}): {error}").into()),
        Ok(path) if status.success() => Ok(path),
        Ok(_) => Err(format!("selected native Cargo build failed: {status}").into()),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_output_must_be_canonically_inside_original_target() {
        let root = std::env::temp_dir().join(format!(
            "conduit-cargo-out-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let target = root.join("target");
        let out = target.join("build/host/out");
        let foreign = root.join("foreign");
        std::fs::create_dir_all(&out).unwrap();
        std::fs::create_dir(&foreign).unwrap();
        assert_eq!(
            checked_out_dir(&out, &target).unwrap(),
            out.canonicalize().unwrap()
        );
        assert!(checked_out_dir(&foreign, &target).is_err());
        assert!(checked_out_dir(&target, &target).is_err());
        #[cfg(unix)]
        {
            let link = target.join("escape");
            std::os::unix::fs::symlink(&foreign, &link).unwrap();
            assert!(checked_out_dir(&link, &target).is_err());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn exact_build_script_package_is_required_even_for_cached_events() {
        let line=br#"{"reason":"build-script-executed","package_id":"path+file:///source#conduit-rp2040-host@0.1.0","out_dir":"/checked/target/build/host/out","fresh":true}"#;
        assert_eq!(
            event_out_dir(line, "conduit-rp2040-host").unwrap(),
            Some("/checked/target/build/host/out".into())
        );
        assert!(event_out_dir(line, "conduit-esp32-host").unwrap().is_none());
        assert!(event_out_dir(br#"{"reason":"build-script-executed","package_id":"path+file:///source#conduit-rp2040-host@0.1.0","out_dir":"relative/out"}"#,"conduit-rp2040-host").is_err());
        assert!(event_out_dir(
            br#"{"reason":"compiler-artifact","package_id":"other"}"#,
            "conduit-rp2040-host"
        )
        .unwrap()
        .is_none());
    }
}
