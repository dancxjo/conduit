use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::doctor::repo_root;
use super::firmware::{pete_capstone_uf2_path, read_firmware_mode, uf2_path};
use super::run_bootsel;
use super::{PicoArgs, PicoResult};

const BOOTSEL_WAIT_SECS: u64 = 90;
const HEADLESS_MOUNT_HELPER: &str = "/usr/local/libexec/conduit-pico-headless-mount";

pub fn run_flash(args: &PicoArgs) -> PicoResult<()> {
    if args.usb_midi_fixture {
        return Err("the USB-MIDI fixture checkpoint is build-only; flashing requires an explicit wiring and physical-acceptance slice".into());
    }
    let root = repo_root();
    let uf2 = if args.pete_capstone {
        pete_capstone_uf2_path(&root)
    } else {
        uf2_path(&root)
    };

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

    let actual_mode = read_firmware_mode(&root)?;
    let expected_mode = if args.indicator_resource {
        "indicator-resource"
    } else if args.pete_capstone {
        "pete-capstone"
    } else if args.distributed_lenia {
        "distributed-lenia"
    } else if args.bluetooth_line {
        "bluetooth-line"
    } else if args.appliance_hello {
        "appliance-hello"
    } else if args.appliance_hil_client {
        "appliance-hil-client"
    } else if args.r1_control {
        "r1-control"
    } else if args.wifi_bootstrap {
        "wifi-bootstrap"
    } else if args.triple_remote {
        "triple-remote"
    } else if args.usb_remote {
        "usb-remote"
    } else {
        "pico-local"
    };
    if actual_mode != expected_mode {
        return Err(format!(
            "refusing to flash {} artifact as {expected_mode}; rebuild with the matching pico build mode",
            actual_mode
        )
        .into());
    }

    if args.indicator_resource {
        super::indicator_build::verify_artifact(&root)?;
    }

    if discover_bootsel_mounts()?.is_empty() {
        match run_bootsel(args) {
            Ok(()) => println!("==> pico flash: waiting for firmware-requested BOOTSEL"),
            Err(error) => println!(
                "==> pico flash: automatic BOOTSEL unavailable ({error}); waiting for a manual BOOTSEL connection"
            ),
        }
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
    release_headless_mount(&mount)?;

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

fn release_headless_mount(mount: &std::path::Path) -> PicoResult<()> {
    if !mount.starts_with("/run/conduit-pico-bootsel/") {
        return Ok(());
    }
    let output = Command::new("sudo")
        .args(["-n", HEADLESS_MOUNT_HELPER, "--unmount"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "headless BOOTSEL cleanup failed after UF2 copy: {}",
            stderr.trim()
        )
        .into());
    }
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

    if let Some(path) = try_headless_mount()? {
        return Ok(path);
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
        if let Some(path) = try_headless_mount()? {
            return Ok(path);
        }
        if let Some(path) = try_udisks_mount()? {
            return Ok(path);
        }
        std::thread::sleep(Duration::from_millis(500));
    }

    Err(format!("timed out waiting for RPI-RP2 volume after {BOOTSEL_WAIT_SECS} seconds").into())
}

fn try_headless_mount() -> PicoResult<Option<PathBuf>> {
    let (_, unmounted) = find_rpi_rp2_devices()?;
    if unmounted.is_empty() || !PathBuf::from(HEADLESS_MOUNT_HELPER).is_file() {
        return Ok(None);
    }

    let output = Command::new("sudo")
        .args(["-n", HEADLESS_MOUNT_HELPER])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "headless BOOTSEL mount helper failed: {}; install it with `sudo targets/rp2040/tools/install-pico-headless-flash.sh`",
            stderr.trim()
        )
        .into());
    }

    let stdout = String::from_utf8(output.stdout)?;
    let path = parse_headless_mount_path(&stdout)?;
    if !path.is_dir() {
        return Err(format!(
            "headless BOOTSEL helper returned a non-directory mount path: {}",
            path.display()
        )
        .into());
    }
    Ok(Some(path))
}

fn parse_headless_mount_path(stdout: &str) -> PicoResult<PathBuf> {
    let mut lines = stdout.lines();
    let path = lines
        .next()
        .filter(|line| !line.is_empty())
        .ok_or("headless BOOTSEL helper returned no mount path")?;
    if lines.next().is_some() || !path.starts_with("/run/conduit-pico-bootsel/") {
        return Err("headless BOOTSEL helper returned an invalid mount path".into());
    }
    Ok(PathBuf::from(path))
}

fn discover_bootsel_mounts() -> PicoResult<Vec<PathBuf>> {
    let (mut mounted, _) = find_rpi_rp2_devices()?;
    mounted.sort();
    mounted.dedup();
    Ok(mounted)
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
        let (mounted, _) = find_rpi_rp2_devices()?;
        if mounted.len() == 1 {
            return Ok(mounted.into_iter().next());
        }
        if mounted.len() > 1 {
            return Err("multiple RPI-RP2 volumes detected after udisks mount".into());
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

#[cfg(test)]
mod tests {
    use super::parse_headless_mount_path;

    #[test]
    fn accepts_only_one_fixed_headless_mount_path() {
        assert_eq!(
            parse_headless_mount_path("/run/conduit-pico-bootsel/1000\n").unwrap(),
            std::path::PathBuf::from("/run/conduit-pico-bootsel/1000")
        );
        assert!(parse_headless_mount_path("").is_err());
        assert!(parse_headless_mount_path("/media/RPI-RP2\n").is_err());
        assert!(parse_headless_mount_path("/run/conduit-pico-bootsel/1000\nextra\n").is_err());
    }
}
