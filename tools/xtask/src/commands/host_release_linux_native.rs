//! Optional SDL linker alias over the selected compiler's existing target SDK.
//! Only ignored build output changes; never installs or rewrites a system library.
use std::{
    ffi::OsString,
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
pub(super) fn linker_search(target: Option<&str>) -> Result<OsString, Box<dyn std::error::Error>> {
    let compiler = if target == Some("aarch64-unknown-linux-gnu") {
        OsString::from("aarch64-linux-gnu-gcc")
    } else {
        std::env::var_os("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER")
            .unwrap_or_else(|| "cc".into())
    };
    let output = Command::new(&compiler)
        .arg("-print-file-name=libSDL2-2.0.so.0")
        .output()?;
    let runtime = String::from_utf8(output.stdout)?;
    let runtime = Path::new(runtime.trim());
    if !output.status.success() || !runtime.is_absolute() || !runtime.is_file() {
        return Err(format!("native-display requires the existing {} SDL2 runtime SDK resolved by {:?}; no cross SDK is installed automatically",target.unwrap_or("x86_64-unknown-linux-gnu"),compiler).into());
    }
    let runtime = runtime.canonicalize()?;
    let root = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target"));
    let directory = root
        .join("host-release-sdl")
        .join(target.unwrap_or("x86_64-unknown-linux-gnu"));
    alias(&runtime, target, &directory)?;
    let mut paths = vec![directory.canonicalize()?];
    if let Some(existing) = std::env::var_os("LIBRARY_PATH") {
        paths.extend(std::env::split_paths(&existing));
    }
    Ok(std::env::join_paths(paths)?)
}
fn alias(
    runtime: &Path,
    target: Option<&str>,
    directory: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = [0u8; 20];
    fs::File::open(runtime)?.read_exact(&mut header)?;
    if !matches_target(&header, target) {
        return Err(format!(
            "native-display SDL2 SDK does not contain a matching {} ELF runtime",
            target.unwrap_or("x86_64-unknown-linux-gnu")
        )
        .into());
    }
    fs::create_dir_all(directory)?;
    let link = directory.join("libSDL2.so");
    match fs::symlink_metadata(&link) {
        Ok(_) => {
            if link.canonicalize()? != runtime {
                return Err("existing SDL2 linker alias selects a different runtime".into());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            std::os::unix::fs::symlink(runtime, &link)?;
            #[cfg(not(unix))]
            return Err("native-display linker alias requires Linux".into());
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}
fn matches_target(header: &[u8; 20], target: Option<&str>) -> bool {
    let machine = match target {
        None | Some("x86_64-unknown-linux-gnu") => 62,
        Some("aarch64-unknown-linux-gnu") => 183,
        _ => return false,
    };
    &header[..4] == b"\x7fELF"
        && header[4] == 2
        && header[5] == 1
        && header[6] == 1
        && u16::from_le_bytes([header[16], header[17]]) == 3
        && u16::from_le_bytes([header[18], header[19]]) == machine
}
#[cfg(test)]
mod tests {
    use super::*;
    fn elf(machine: u16) -> [u8; 20] {
        let mut h = [0; 20];
        h[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        h[16..18].copy_from_slice(&3u16.to_le_bytes());
        h[18..20].copy_from_slice(&machine.to_le_bytes());
        h
    }
    #[test]
    fn target_sdk_refusal_precedes_alias_directory_creation() {
        let root = std::env::temp_dir().join(format!(
            "conduit-sdl-alias-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let runtime = root.join("runtime.so");
        fs::write(&runtime, elf(62)).unwrap();
        let aliases = root.join("aliases");
        assert!(alias(&runtime, Some("aarch64-unknown-linux-gnu"), &aliases).is_err());
        assert!(!aliases.exists());
        #[cfg(unix)]
        {
            alias(&runtime, None, &aliases).unwrap();
            alias(&runtime, None, &aliases).unwrap();
            let other = root.join("other.so");
            fs::write(&other, elf(62)).unwrap();
            assert!(alias(&other, None, &aliases).is_err());
            assert_eq!(
                aliases.join("libSDL2.so").canonicalize().unwrap(),
                runtime.canonicalize().unwrap()
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
