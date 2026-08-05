use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

use super::doctor::repo_root;
use super::firmware::uf2_path;
use super::{PicoArgs, PicoResult};

const RPI_RP2_LABEL: &str = "RPI-RP2";
const BOOTSEL_WAIT_SECS: u64 = 90;

pub fn run_flash(args: &PicoArgs) -> PicoResult<()> {
    let root = repo_root();
    let uf2 = uf2_path(&root);

    if args.dry_run {
        println!("==> pico flash (dry-run)");
        println!("  UF2 source: {}", uf2.display());
        println!(
            "  mount candidate: {}",
            args.mount
                .as_deref()
                .unwrap_or("<auto-discover RPI-RP2>")
        );
        return Ok(());
    }

    if !uf2.exists() {
        return Err(format!("UF2 not found at {}; run `cargo xtask pico build` first", uf2.display()).into());
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
        return Err(format!("specified mount path is not a directory: {}", path.display()).into());
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

    println!("No RPI-RP2 volume detected. Hold BOOTSEL while connecting the Pico W, then press Enter.");
    let mut line = String::new();
    std::io::stdin().read_line(&mut line)?;

    let deadline = Instant::now() + Duration::from_secs(BOOTSEL_WAIT_SECS);
    while Instant::now() < deadline {
        let candidates = discover_bootsel_mounts()?;
        if candidates.len() == 1 {
            return Ok(candidates[0].clone());
        }
        if candidates.len() > 1 {
            return Err("multiple RPI-RP2 volumes detected; pass --mount".into());
        }
        std::thread::sleep(Duration::from_secs(2));
    }

    Err(format!("timed out waiting for RPI-RP2 after {BOOTSEL_WAIT_SECS} seconds").into())
}

fn discover_bootsel_mounts() -> PicoResult<Vec<PathBuf>> {
    let mut candidates = standard_paths()
        .into_iter()
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();

    let output = Command::new("lsblk")
        .args(["-rnpo", "LABEL,PATH,MOUNTPOINT,FSTYPE"])
        .output()?;
    if output.status.success() {
        let listing = String::from_utf8(output.stdout)?;
        for line in listing.lines() {
            let fields = line.split_whitespace().collect::<Vec<_>>();
            if fields.first() == Some(&RPI_RP2_LABEL)
                && fields.get(3) == Some(&"vfat")
                && fields.get(2).is_some_and(|mount| *mount != "-")
            {
                let path = PathBuf::from(fields[2]);
                if path.is_dir() && !candidates.contains(&path) {
                    candidates.push(path);
                }
            }
        }
    }

    candidates.sort();
    candidates.dedup();
    Ok(candidates)
}

fn try_udisks_mount() -> PicoResult<Option<PathBuf>> {
    let output = Command::new("lsblk")
        .args(["-rnpo", "LABEL,PATH,MOUNTPOINT,FSTYPE"])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let listing = String::from_utf8(output.stdout)?;
    let block = listing.lines().find_map(|line| {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        (fields.first() == Some(&RPI_RP2_LABEL)
            && fields.get(3) == Some(&"vfat")
            && fields.get(2).is_some_and(|mount| *mount == "-"))
        .then(|| fields.get(1).copied())
        .flatten()
    });
    let Some(block) = block else {
        return Ok(None);
    };

    let output = Command::new("udisksctl")
        .args(["mount", "-b", block])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(output.stdout)?;
    Ok(text
        .split_once(" at ")
        .map(|(_, mount)| PathBuf::from(mount.trim().trim_end_matches('.'))))
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
