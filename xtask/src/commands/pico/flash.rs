use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::doctor::repo_root;
use super::firmware::uf2_path;
use super::{PicoArgs, PicoResult};

const BOOTSEL_WAIT_SECS: u64 = 90;

pub fn run_flash(args: &PicoArgs) -> PicoResult<()> {
    let root = repo_root();
    let uf2 = uf2_path(&root);

    if args.dry_run {
        println!("==> pico flash (dry-run)");
        println!("  UF2 source: {}", uf2.display());
        println!(
            "  mount candidate: {}",
            args.mount.as_deref().unwrap_or("<auto-discover RPI-RP2>")
        );
        return Ok(());
    }

    if !uf2.exists() {
        return Err(format!(
            "UF2 not found at {}; run `cargo xtask pico build` first",
            uf2.display()
        )
        .into());
    }

    let mount = resolve_mount(args)?;
    let destination = mount.join(
        uf2.file_name()
            .ok_or("UF2 path does not contain a file name")?,
    );
    println!("==> pico flash: copying UF2 to {}", destination.display());
    std::fs::copy(&uf2, &destination)?;
    let status = Command::new("sync").status()?;
    if !status.success() {
        return Err("sync failed after UF2 copy".into());
    }

    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if !mount.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    println!("==> pico flash: done");
    Ok(())
}

fn resolve_mount(args: &PicoArgs) -> PicoResult<PathBuf> {
    if let Some(mount) = &args.mount {
        let path = PathBuf::from(mount);
        if path.is_dir() {
            return Ok(path);
        }
        return Err(format!(
            "specified mount path is not a directory: {}",
            path.display()
        )
        .into());
    }

    let candidates = discover_bootsel_mounts()?;
    match candidates.len() {
        1 => return Ok(candidates[0].clone()),
        n if n > 1 => {
            return Err(format!(
                "multiple RPI-RP2 volumes detected: {}; pass --mount",
                candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .into())
        }
        _ => {}
    }

    if let Some(path) = try_udisks_mount()? {
        return Ok(path);
    }

    println!(
        "==> Waiting up to {BOOTSEL_WAIT_SECS} seconds for RPI-RP2 volume (hold BOOTSEL while connecting Pico W)..."
    );
    let deadline = Instant::now() + Duration::from_secs(BOOTSEL_WAIT_SECS);
    while Instant::now() < deadline {
        let candidates = discover_bootsel_mounts()?;
        if candidates.len() == 1 {
            return Ok(candidates[0].clone());
        }
        if candidates.len() > 1 {
            return Err("multiple RPI-RP2 volumes detected; pass --mount".into());
        }
        if let Some(path) = try_udisks_mount()? {
            return Ok(path);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!("timed out waiting for RPI-RP2 volume after {BOOTSEL_WAIT_SECS} seconds").into())
}

fn discover_bootsel_mounts() -> PicoResult<Vec<PathBuf>> {
    let mut candidates = standard_paths()
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();

    let (mounted, _) = find_rpi_rp2_devices()?;
    for path in mounted {
        if !candidates.contains(&path) {
            candidates.push(path);
        }
    }

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn try_udisks_mount() -> PicoResult<Option<PathBuf>> {
    let (_, unmounted) = find_rpi_rp2_devices()?;
    for block in unmounted {
        let output = Command::new("udisksctl")
            .args(["mount", "-b", block.to_str().unwrap_or_default()])
            .output()?;
        if output.status.success() {
            let text = String::from_utf8(output.stdout)?;
            if let Some((_, mount)) = text.split_once(" at ") {
                let path = PathBuf::from(mount.trim().trim_end_matches('.'));
                if path.is_dir() {
                    return Ok(Some(path));
                }
            }
        }
        for path in standard_paths() {
            if path.is_dir() {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
}

fn find_rpi_rp2_devices() -> PicoResult<(Vec<PathBuf>, Vec<PathBuf>)> {
    let output = Command::new("lsblk")
        .args(["-P", "-o", "LABEL,PATH,MOUNTPOINT,FSTYPE"])
        .output()?;
    let mut mounted = Vec::new();
    let mut unmounted = Vec::new();

    if output.status.success() {
        let listing = String::from_utf8(output.stdout)?;
        for line in listing.lines() {
            let is_rpi_rp2 = line.contains("LABEL=\"RPI-RP2\"")
                || line.contains("LABEL=\"rpi-rp2\"")
                || line.contains("LABEL=\"BOOTSEL\"");
            let is_vfat = line.contains("FSTYPE=\"vfat\"") || line.contains("FSTYPE=\"msdos\"");
            if is_rpi_rp2 && is_vfat {
                if let Some(path_str) = extract_kv(line, "PATH") {
                    if let Some(mount_str) = extract_kv(line, "MOUNTPOINT") {
                        if !mount_str.is_empty() && mount_str != "-" {
                            let mount_path = PathBuf::from(mount_str);
                            if mount_path.is_dir() {
                                mounted.push(mount_path);
                            }
                        } else {
                            unmounted.push(PathBuf::from(path_str));
                        }
                    }
                }
            }
        }
    }

    Ok((mounted, unmounted))
}

fn extract_kv(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}

fn standard_paths() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    vec![
        PathBuf::from(format!("/run/media/{user}/RPI-RP2")),
        PathBuf::from(format!("/media/{user}/RPI-RP2")),
        PathBuf::from("/media/RPI-RP2"),
        PathBuf::from("/Volumes/RPI-RP2"),
    ]
}
