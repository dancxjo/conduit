use anyhow::{bail, Result};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::doctor::repo_root;
use super::firmware::uf2_path;
use super::PicoArgs;

const RPI_RP2_LABEL: &str = "RPI-RP2";
const BOOTSEL_WAIT_SECS: u64 = 90;

pub fn run_flash(args: &PicoArgs) -> Result<()> {
    let root = repo_root();
    let uf2 = uf2_path(&root);

    if args.dry_run {
        println!("==> pico flash (dry-run)");
        println!("  UF2 source: {}", uf2.display());
        println!("  mount candidate: {}", args.mount.as_deref().unwrap_or("<auto-discover RPI-RP2>"));
        return Ok(());
    }

    if !uf2.exists() {
        bail!("UF2 not found at {}. Run `pico build` first.", uf2.display());
    }

    let mount = resolve_mount(args)?;
    println!("==> pico flash: copying UF2 to {}", mount.display());
    let dest = mount.join(uf2.file_name().unwrap());
    std::fs::copy(&uf2, &dest)
        .map_err(|e| anyhow::anyhow!("failed to copy UF2: {}", e))?;
    let _ = Command::new("sync").status();
    println!("==> pico flash: UF2 written; waiting for Pico to reset...");

    // Wait up to 10 s for the BOOTSEL volume to disappear (indicates successful reset)
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

fn resolve_mount(args: &PicoArgs) -> Result<PathBuf> {
    // 1. Explicit CLI/env
    if let Some(m) = &args.mount {
        let p = PathBuf::from(m);
        if p.exists() {
            return Ok(p);
        }
        bail!("Specified mount path does not exist: {}", p.display());
    }

    // 2. Auto-discover via lsblk
    if let Some(p) = discover_bootsel_mount()? {
        return Ok(p);
    }

    // 3. Try udisksctl auto-mount
    if let Some(p) = try_udisks_mount()? {
        return Ok(p);
    }

    // 4. Prompt operator to connect in BOOTSEL mode
    println!(
        "No RPI-RP2 volume detected. Hold BOOTSEL while connecting the Pico W, then press Enter."
    );
    let mut _line = String::new();
    std::io::stdin().read_line(&mut _line).ok();

    let deadline = Instant::now() + Duration::from_secs(BOOTSEL_WAIT_SECS);
    loop {
        if Instant::now() > deadline {
            bail!("Timed out waiting for RPI-RP2 BOOTSEL volume after {} seconds.", BOOTSEL_WAIT_SECS);
        }
        if let Some(p) = discover_bootsel_mount()? {
            return Ok(p);
        }
        std::thread::sleep(Duration::from_secs(2));
    }
}

fn discover_bootsel_mount() -> Result<Option<PathBuf>> {
    // Standard Linux paths
    for candidate in standard_linux_paths() {
        if candidate.is_dir() {
            return Ok(Some(candidate));
        }
    }

    // lsblk JSON probe
    let output = Command::new("lsblk")
        .args(["-J", "-o", "NAME,LABEL,MOUNTPOINT,FSTYPE"])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(None),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    // Simple text search — avoid pulling in a JSON crate just for this
    if text.contains(RPI_RP2_LABEL) {
        for line in text.lines() {
            if line.contains("mountpoint") && !line.contains("null") {
                // Extract mountpoint value between quotes
                if let Some(start) = line.rfind('"') {
                    let candidate = line[..start].rsplit('"').next().unwrap_or("").trim();
                    if !candidate.is_empty() {
                        return Ok(Some(PathBuf::from(candidate)));
                    }
                }
            }
        }
    }
    Ok(None)
}

fn try_udisks_mount() -> Result<Option<PathBuf>> {
    // Find unmounted RPI-RP2 device and mount it
    let output = Command::new("lsblk")
        .args(["-J", "-o", "NAME,LABEL,MOUNTPOINT"])
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok(None),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    if !text.contains(RPI_RP2_LABEL) {
        return Ok(None);
    }
    // Find the device name for RPI-RP2 with null mountpoint
    let mut device = None;
    let mut in_rpi = false;
    for line in text.lines() {
        if line.contains(RPI_RP2_LABEL) {
            in_rpi = true;
        }
        if in_rpi && line.contains("\"name\"") {
            if let Some(start) = line.find('"') {
                let rest = &line[start + 1..];
                if let Some(end) = rest.find('"') {
                    device = Some(rest[..end].to_string());
                }
            }
        }
        if in_rpi && line.contains("\"mountpoint\": null") && device.is_some() {
            break;
        }
    }
    let dev = match device {
        Some(d) => d,
        None => return Ok(None),
    };
    println!("==> pico flash: mounting /dev/{} via udisksctl", dev);
    let result = Command::new("udisksctl")
        .args(["mount", "-b", &format!("/dev/{}", dev)])
        .output();
    match result {
        Ok(o) if o.status.success() => {
            let out = String::from_utf8_lossy(&o.stdout);
            // udisksctl prints: Mounted /dev/sdX at /run/media/...
            if let Some(pos) = out.find("at ") {
                let path = out[pos + 3..].trim().to_string();
                return Ok(Some(PathBuf::from(path)));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn standard_linux_paths() -> Vec<PathBuf> {
    let user = std::env::var("USER").unwrap_or_default();
    vec![
        PathBuf::from(format!("/run/media/{}/RPI-RP2", user)),
        PathBuf::from("/media/RPI-RP2"),
        PathBuf::from("/mnt/RPI-RP2"),
        PathBuf::from("/Volumes/RPI-RP2"), // macOS
    ]
}
