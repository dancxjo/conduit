use std::path::{Path, PathBuf};
use std::process::Command;

use crate::process::command_for;

use super::PicoResult;

const REQUIRED_TOOLS: &[(&str, &str)] = &[
    ("cargo", "Install Rust with rustup"),
    ("rustc", "Install Rust with rustup"),
    ("rustup", "Install Rust with rustup"),
    ("elf2uf2-rs", "cargo install elf2uf2-rs --locked"),
    ("sha256sum", "Install GNU coreutils"),
    ("curl", "Install curl"),
    ("lsblk", "Install util-linux on Linux"),
    ("sync", "Install coreutils"),
];

const OPTIONAL_TOOLS: &[(&str, &str)] = &[("udisksctl", "optional BOOTSEL auto-mount helper")];

const REQUIRED_TARGET: &str = "thumbv6m-none-eabi";

pub const CYW43_ASSETS: &[(&str, &str)] = &[
    (
        "43439A0.bin",
        "5555e0261da2610a500d68c18d895cace0152bbefbf76f4aa683ebce77e3d7eb",
    ),
    (
        "43439A0_clm.bin",
        "e712b3d218e8b1e2747b092e03b8b0afcb8c8c8e355d2a4a0d47b493800f3f89",
    ),
    (
        "43439A0_btfw.bin",
        "ce1992c1a6a16ae51bc012439486e9fb212623eca92d9e82a8090c2acf7ef1df",
    ),
    (
        "nvram_rp2040.bin",
        "4904bdbb0c937bd0ac2eb2a1d62f2da4dd90e32082384e02874e8d671b0f330d",
    ),
    (
        "LICENSE-permissive-binary-license-1.0.txt",
        "5f65b8a496ac27afda41917c18cb6e690b4a022df1f5a12ea823eb38a287f50e",
    ),
];

pub const CYW43_COMMIT: &str = "6a823b96b3d270b6da1cc667f8acea749e588dab";
pub const CYW43_ASSET_DIR: &str =
    "targets/rp2040/firmware/assets/cyw43/embassy-6a823b96b3d270b6da1cc667f8acea749e588dab";

pub fn run_doctor(dry_run: bool) -> PicoResult<()> {
    println!("==> pico doctor: checking prerequisites");
    let mut missing = Vec::new();

    for (tool, repair) in REQUIRED_TOOLS {
        let found = which_tool(tool);
        println!("  {} {}", if found { "✓" } else { "✗" }, tool);
        if !found {
            println!("      fix: {repair}");
            missing.push(*tool);
        }
    }

    for (tool, note) in OPTIONAL_TOOLS {
        println!(
            "  {} {} (optional) — {}",
            if which_tool(tool) { "✓" } else { "-" },
            tool,
            note
        );
    }

    let target_ok = check_rustup_target(REQUIRED_TARGET);
    println!(
        "  {} rustup target {}",
        if target_ok { "✓" } else { "✗" },
        REQUIRED_TARGET
    );
    if !target_ok {
        println!("      fix: rustup target add {REQUIRED_TARGET}");
        missing.push(REQUIRED_TARGET);
    }

    let asset_dir = repo_root().join(CYW43_ASSET_DIR);
    println!(
        "==> pico doctor: verifying vendored CYW43 assets in {}",
        asset_dir.display()
    );
    let mut asset_ok = true;
    for (filename, expected_hex) in CYW43_ASSETS {
        let path = asset_dir.join(filename);
        if dry_run {
            println!("  planned: verify {}", path.display());
            continue;
        }
        match verify_sha256(&path, expected_hex) {
            Ok(()) => println!("  ✓ {filename}"),
            Err(error) => {
                println!("  ✗ {filename} — {error}");
                asset_ok = false;
            }
        }
    }

    if dry_run {
        println!("==> pico doctor: dry run complete");
        return Ok(());
    }
    if !missing.is_empty() {
        return Err(format!("missing required tools: {}", missing.join(", ")).into());
    }
    if !asset_ok {
        return Err(
            "CYW43 asset verification failed; run `cargo xtask pico --refresh-radio-assets`".into(),
        );
    }

    println!("==> pico doctor: all checks passed");
    Ok(())
}

fn which_tool(name: &str) -> bool {
    let probe_arg = match name {
        "elf2uf2-rs" => "--help",
        _ => "--version",
    };

    command_for(name)
        .arg(probe_arg)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn check_rustup_target(target: &str) -> bool {
    Command::new("rustup")
        .args(["target", "list", "--installed"])
        .output()
        .map(|output| {
            output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .any(|line| line.trim() == target)
        })
        .unwrap_or(false)
}

pub fn sha256_file(path: &Path) -> PicoResult<String> {
    let output = Command::new("sha256sum").arg(path).output()?;
    if !output.status.success() {
        return Err(format!("sha256sum failed for {}", path.display()).into());
    }
    let text = String::from_utf8(output.stdout)?;
    text.split_whitespace()
        .next()
        .map(str::to_owned)
        .ok_or_else(|| format!("sha256sum returned no digest for {}", path.display()).into())
}

pub fn verify_sha256(path: &Path, expected_hex: &str) -> PicoResult<()> {
    let actual = sha256_file(path)?;
    if actual != expected_hex {
        return Err(format!(
            "SHA-256 mismatch for {}: expected {}, actual {}",
            path.display(),
            expected_hex,
            actual
        )
        .into());
    }
    Ok(())
}

pub fn repo_root() -> PathBuf {
    crate::workspace::workspace_root()
        .unwrap_or_else(|_| std::env::current_dir().expect("current directory must be available"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_asset_table_is_complete_and_unique() {
        assert_eq!(CYW43_ASSETS.len(), 5);
        let mut names = CYW43_ASSETS
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), CYW43_ASSETS.len());
        assert!(CYW43_ASSETS.iter().all(|(_, digest)| digest.len() == 64));
    }
}
