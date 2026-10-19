//! Reviewed official HTTPS pins, NOT successful verification of the floating index.
//! GCC/OTA: Arduino/Arduino a0df6e0e83b652c72bc78b0a1376c54d6ebc3bee,
//! hardware/package_index_bundled.json. avrdude: official avrdude-packing release
//! 8.0-arduino.1/package_index_draft.json. SparkFun Arduino_Boards commit
//! b2ccf64208ea990ce764fd60b109931296ad1350. Core archive matches all 549 regular
//! files of ArduinoCore-avr commit 86df345b3cf46754a5db38fb983ec2808ce31303;
//! ten source-only workflow/packaging files are omitted, no changed/extra files.
use super::sha256_file;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
#[derive(Clone, Copy)]
struct Archive {
    identity: &'static str,
    url: &'static str,
    sha256: &'static str,
    prefix: &'static str,
    destination: &'static str,
}
fn archives() -> Result<Vec<Archive>> {
    let mut entries = [
Archive {
    identity: "arduino/avr/1.8.8",
    url: "https://downloads.arduino.cc/cores/staging/avr-1.8.8.tar.bz2",
    sha256: "a234c2a43dcd01ce54be665806f183b8e6ec4e966d2e3e0c3358b63023d6390c",
    prefix: "ArduinoCore-avr-1.8.8",
    destination: "arduino/hardware/avr/1.8.8",
},
Archive {
    identity: "SparkFun/avr/1.1.13",
    url: "https://raw.githubusercontent.com/sparkfun/Arduino_Boards/b2ccf64208ea990ce764fd60b109931296ad1350/IDE_Board_Manager/sparkfunboards.1.1.13.tar.bz2",
    sha256: "d7af391cafc5e16830cac7c13484ef62765dd7a36aaba5f25020ce3c39617115",
    prefix: "avr",
    destination: "SparkFun/hardware/avr/1.1.13",
},
 ].to_vec();
    entries.extend(match std::env::consts::ARCH {
"x86_64" => [
Archive {
    identity: "arduino/avr-gcc/7.3.0-atmel3.6.1-arduino7",
    url: "https://downloads.arduino.cc/tools/avr-gcc-7.3.0-atmel3.6.1-arduino7-x86_64-pc-linux-gnu.tar.bz2",
    sha256: "bd8c37f6952a2130ac9ee32c53f6a660feb79bee8353c8e289eb60fdcefed91e",
    prefix: "avr",
    destination: "arduino/tools/avr-gcc/7.3.0-atmel3.6.1-arduino7",
},
Archive {
    identity: "arduino/arduinoOTA/1.3.0",
    url: "https://downloads.arduino.cc/tools/arduinoOTA-1.3.0-linux_amd64.tar.bz2",
    sha256: "aa45ee2441ffc3a122daec5802941d1fa2ac47adf5c5c481b5e0daa4dc259ffa",
    prefix: "arduinoOTA",
    destination: "arduino/tools/arduinoOTA/1.3.0",
},
Archive {
    identity: "arduino/avrdude/8.0.0-arduino1",
    url: "https://downloads.arduino.cc/tools/avrdude_8.0-arduino.1_Linux_64bit.tar.gz",
    sha256: "1d2edadb07e070fcd016e8c802e961e30bfa606c2db63625c930526e7044aaa6",
    prefix: "avrdude_Linux_64bit",
    destination: "arduino/tools/avrdude/8.0.0-arduino1",
},
],
"aarch64" => [
Archive {
    identity: "arduino/avr-gcc/7.3.0-atmel3.6.1-arduino7",
    url: "https://downloads.arduino.cc/tools/avr-gcc-7.3.0-atmel3.6.1-arduino7-aarch64-pc-linux-gnu.tar.bz2",
    sha256: "03d322b9df6da17289e9e7c6233c34a8535d9c645c19efc772ba19e56914f339",
    prefix: "avr",
    destination: "arduino/tools/avr-gcc/7.3.0-atmel3.6.1-arduino7",
},
Archive {
    identity: "arduino/arduinoOTA/1.3.0",
    url: "https://downloads.arduino.cc/tools/arduinoOTA-1.3.0-linux_arm64.tar.bz2",
    sha256: "835ed8f37cffac37e979d1b0f6041559592d3d98be52f0e8611b76c4858e4113",
    prefix: "arduinoOTA",
    destination: "arduino/tools/arduinoOTA/1.3.0",
},
Archive {
    identity: "arduino/avrdude/8.0.0-arduino1",
    url: "https://downloads.arduino.cc/tools/avrdude_8.0-arduino.1_Linux_ARM64.tar.gz",
    sha256: "031f212bdcb77e2ea13895f07ab4a0c254963f8f1bbd3b8797d3363a6a2e95e3",
    prefix: "avrdude_Linux_ARM64",
    destination: "arduino/tools/avrdude/8.0.0-arduino1",
},
],
_ => return Err("pinned AVR archives support x86_64/aarch64 only".into()),
 });
    Ok(entries)
}
pub(super) fn identities() -> Result<BTreeMap<String, String>> {
    Ok(archives()?
        .into_iter()
        .map(|a| (a.identity.into(), format!("{}#sha256={}", a.url, a.sha256)))
        .collect())
}
pub(super) fn install(root: &Path) -> Result<()> {
    if std::env::consts::OS != "linux" {
        return Err("pinned AVR installation supports Linux only".into());
    }
    let data = root.join("target/avr-promicro/arduino/data");
    checked_directory(&data)?;
    let packages = data.join("packages");
    if fs::symlink_metadata(&packages).is_ok() {
        return Err("unreceipted AVR packages exist; preserving unknown installation".into());
    }
    let cache = root.join("target/avr-promicro/arduino/downloads/pinned-archives");
    checked_directory(&cache)?;
    let staging = data.join(format!(
        ".pinned-core-staging-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos()
    ));
    fs::create_dir(&staging)?;
    // Unique failed stages remain available for diagnosis; never replace unknown work.
    for archive in archives()? {
        let file = download(&cache, archive)?;
        validate_archive(&file, archive.prefix)?;
        let destination = staging.join(archive.destination);
        fs::create_dir_all(&destination)?;
        let output = Command::new("tar")
            .args([
                "--extract",
                "--no-same-owner",
                "--no-same-permissions",
                "--delay-directory-restore",
                "--strip-components=1",
                "--file",
            ])
            .arg(&file)
            .arg("--directory")
            .arg(&destination)
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "pinned AVR extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
    }
    if fs::symlink_metadata(&packages).is_ok() {
        return Err("AVR packages appeared during staging; preserving both trees".into());
    }
    fs::rename(staging, packages)?;
    Ok(())
}
fn checked_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            checked_directory(parent)?;
        }
    }
    if let Ok(m) = fs::symlink_metadata(path) {
        if !m.is_dir() || m.file_type().is_symlink() {
            return Err(format!("unsafe AVR directory: {}", path.display()).into());
        }
    } else {
        fs::create_dir(path)?;
    }
    Ok(())
}
fn verified_archive(file: &Path, expected: &str) -> Result<()> {
    let m = fs::symlink_metadata(file)?;
    if !m.is_file() || m.file_type().is_symlink() || m.len() > 64 * 1024 * 1024 {
        return Err("AVR archive is not a bounded regular file".into());
    }
    if sha256_file(file)? != expected {
        return Err(format!("AVR archive digest mismatch; preserving {}", file.display()).into());
    }
    Ok(())
}
fn download(cache: &Path, archive: Archive) -> Result<PathBuf> {
    let name = archive
        .url
        .rsplit('/')
        .next()
        .ok_or("archive filename absent")?;
    let file = cache.join(name);
    if fs::symlink_metadata(&file).is_ok() {
        verified_archive(&file, archive.sha256)?;
        return Ok(file);
    }
    let temporary = cache.join(format!("{name}.download-{}", std::process::id()));
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let output = Command::new("curl")
        .args([
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--connect-timeout",
            "15",
            "--max-time",
            "180",
            "--retry",
            "1",
            "--max-filesize",
            "67108864",
            "--output",
        ])
        .arg(&temporary)
        .arg(archive.url)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "AVR archive download failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    verified_archive(&temporary, archive.sha256)?;
    if fs::symlink_metadata(&file).is_ok() {
        return Err("AVR cache entry appeared during download".into());
    }
    fs::rename(temporary, &file)?;
    Ok(file)
}
fn member_path(name: &str, prefix: &str) -> Result<Vec<String>> {
    if name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.chars().any(char::is_control)
    {
        return Err("unsafe AVR archive member spelling".into());
    }
    let parts: Vec<_> = name.trim_end_matches('/').split('/').collect();
    if parts.first().copied() != Some(prefix)
        || parts
            .iter()
            .any(|p| p.is_empty() || *p == "." || *p == "..")
    {
        return Err("AVR member escapes its reviewed single prefix".into());
    }
    Ok(parts.into_iter().map(String::from).collect())
}
fn link_target(name: &str, target: &str, prefix: &str, hard: bool) -> Result<()> {
    if hard {
        member_path(target, prefix)?;
        return Ok(());
    }
    if target.is_empty()
        || target.starts_with('/')
        || target.contains('\\')
        || target.chars().any(char::is_control)
    {
        return Err("unsafe AVR link target".into());
    }
    let mut parts = member_path(name, prefix)?;
    parts.pop();
    for p in target.split('/') {
        match p {
            "" | "." => (),
            ".." => {
                if parts.len() <= 1 {
                    return Err("AVR link escapes prefix".into());
                }
                parts.pop();
            }
            _ => parts.push(p.into()),
        }
    }
    Ok(())
}
fn validate_listing(listing: &str, prefix: &str) -> Result<()> {
    let mut members = Vec::new();
    let mut links = Vec::new();
    let mut bytes = 0_u64;
    for line in listing.lines() {
        let f: Vec<_> = line.split_whitespace().collect();
        if f.len() < 6 {
            return Err("malformed AVR archive listing".into());
        }
        let kind = f[0].as_bytes()[0];
        // GNU tar emits five metadata fields, then an unquoted literal name.
        // Space-bearing official core filenames must remain valid.
        let mut offset = 0;
        for _ in 0..5 {
            let rest = &line[offset..];
            offset += rest
                .find(|c: char| !c.is_whitespace())
                .ok_or("missing tar field")?;
            offset += line[offset..]
                .find(char::is_whitespace)
                .ok_or("missing tar filename")?;
        }
        let payload = line[offset..].trim_start();
        let name = match kind {
            b'l' => {
                payload
                    .split_once(" -> ")
                    .ok_or("malformed symlink listing")?
                    .0
            }
            b'h' => {
                payload
                    .split_once(" link to ")
                    .ok_or("malformed hardlink listing")?
                    .0
            }
            _ => payload,
        };
        member_path(name, prefix)?;
        bytes = bytes
            .checked_add(f[2].parse::<u64>()?)
            .ok_or("archive size overflow")?;
        if bytes > 512 * 1024 * 1024 || members.len() >= 10_000 {
            return Err("AVR expansion exceeds finite bound".into());
        }
        match kind {
            b'-' | b'd' => (),
            b'l' => {
                let target = payload.split_once(" -> ").ok_or("symlink target absent")?.1;
                link_target(name, target, prefix, false)?;
                links.push(name.trim_end_matches('/').to_owned());
            }
            b'h' => {
                let target = payload
                    .split_once(" link to ")
                    .ok_or("hardlink target absent")?
                    .1;
                link_target(name, target, prefix, true)?;
                links.push(name.trim_end_matches('/').to_owned());
            }
            _ => return Err("unsupported AVR member or ambiguous filename".into()),
        }
        if members
            .iter()
            .any(|old: &String| old == name.trim_end_matches('/'))
        {
            return Err("duplicate AVR archive member".into());
        }
        members.push(name.trim_end_matches('/').to_owned());
    }
    if members.is_empty() {
        return Err("empty AVR archive".into());
    }
    for link in links {
        if members.iter().any(|m| m.starts_with(&format!("{link}/"))) {
            return Err("AVR member traverses link ancestor".into());
        }
    }
    Ok(())
}
fn validate_archive(file: &Path, prefix: &str) -> Result<()> {
    let output = Command::new("tar")
        .args([
            "--list",
            "--verbose",
            "--numeric-owner",
            "--quoting-style=literal",
            "--file",
        ])
        .arg(file)
        .env("LC_ALL", "C")
        .output()?;
    if !output.status.success() || output.stdout.len() > 4 * 1024 * 1024 {
        return Err("AVR archive inspection failed or exceeded bound".into());
    }
    validate_listing(std::str::from_utf8(&output.stdout)?, prefix)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "linux")]
    #[test]
    fn unreceipted_installation_is_preserved_before_any_download() {
        let root = std::env::temp_dir().join(format!("conduit-avr-unknown-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let packages = root.join("target/avr-promicro/arduino/data/packages");
        fs::create_dir_all(&packages).unwrap();
        let sentinel = packages.join("unknown-content");
        fs::write(&sentinel, b"preserve").unwrap();
        assert!(install(&root)
            .unwrap_err()
            .to_string()
            .contains("preserving unknown"));
        assert_eq!(fs::read(&sentinel).unwrap(), b"preserve");
        assert!(!root.join("target/avr-promicro/arduino/downloads").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsafe_members_links_and_special_files_refuse_before_extraction() {
        for n in [
            "/avr/file",
            "avr/../file",
            "other/file",
            "avr/a\tb",
            "avr//file",
        ] {
            assert!(member_path(n, "avr").is_err());
        }
        assert!(link_target("avr/lib/link", "../../outside", "avr", false).is_err());
        assert!(link_target("avr/lib/link", "../safe", "avr", false).is_ok());
        assert!(validate_listing(
            "-rw-r--r-- 0/0 4 2020-01-01 00:00 avr/with spaces.pdf",
            "avr"
        )
        .is_ok());
        assert!(link_target("avr/bin/link", "other/file", "avr", true).is_err());
        assert!(validate_listing("crw-r--r-- 0/0 0 2020-01-01 00:00 avr/device", "avr").is_err());
        assert!(validate_listing("lrwxrwxrwx 0/0 0 2020-01-01 00:00 avr/link -> safe\n-rw-r--r-- 0/0 1 2020-01-01 00:00 avr/link/child","avr").is_err());
    }
    #[test]
    fn corrupt_cache_digest_refuses_before_extraction() {
        let file = std::env::temp_dir().join(format!("conduit-avr-corrupt-{}", std::process::id()));
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file)
            .unwrap();
        assert!(verified_archive(&file, &"0".repeat(64)).is_err());
        fs::remove_file(file).unwrap();
    }
    #[test]
    fn manifest_contains_five_unique_reviewed_archives() {
        let m = identities().unwrap();
        assert_eq!(m.len(), 5);
        assert!(m
            .values()
            .all(|v| v.starts_with("https://") && v.contains("#sha256=")));
    }
}
