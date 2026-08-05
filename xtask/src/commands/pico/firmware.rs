use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::doctor::{repo_root, verify_sha256, CYW43_ASSETS, CYW43_ASSET_DIR, CYW43_COMMIT};
use super::PicoArgs;

pub const FIRMWARE_PACKAGE: &str = "conduit-pico-w-signal";
pub const TARGET: &str = "thumbv6m-none-eabi";
pub const PROFILE: &str = "release";

#[derive(Debug, Serialize, Deserialize)]
pub struct FirmwareIdentity {
    pub schema: String,
    pub git_revision: String,
    pub target: String,
    pub profile: String,
    pub firmware_sha256: String,
    pub cyw43_commit: String,
    pub cyw43_assets: Vec<AssetEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssetEntry {
    pub filename: String,
    pub sha256: String,
}

pub fn run_build(args: &PicoArgs) -> Result<()> {
    println!("==> pico build: verifying CYW43 assets");
    let root = repo_root();
    let asset_dir = root.join(CYW43_ASSET_DIR);
    for (filename, expected) in CYW43_ASSETS {
        let p = asset_dir.join(filename);
        if args.dry_run {
            println!("  (dry-run) would verify {}", p.display());
        } else {
            verify_sha256(&p, expected)?;
        }
    }

    println!("==> pico build: building firmware");
    let manifest = root.join("firmware").join(FIRMWARE_PACKAGE).join("Cargo.toml");
    let cmd_args = [
        "build",
        "--manifest-path",
        manifest.to_str().unwrap(),
        "--package",
        FIRMWARE_PACKAGE,
        "--target",
        TARGET,
        "--release",
    ];
    println!("  cargo {}", cmd_args.join(" "));
    if !args.dry_run {
        let status = Command::new("cargo")
            .args(&cmd_args)
            .status()
            .map_err(|e| anyhow::anyhow!("failed to run cargo: {}", e))?;
        if !status.success() {
            bail!("cargo build failed");
        }
    }

    let elf = root
        .join("target")
        .join(TARGET)
        .join(PROFILE)
        .join(FIRMWARE_PACKAGE);
    let uf2 = elf.with_extension("uf2");

    println!("==> pico build: converting ELF to UF2");
    let uf2_cmd = ["elf2uf2-rs", elf.to_str().unwrap(), uf2.to_str().unwrap()];
    println!("  {}", uf2_cmd.join(" "));
    if !args.dry_run {
        let status = Command::new("elf2uf2-rs")
            .arg(&elf)
            .arg(&uf2)
            .status()
            .map_err(|e| anyhow::anyhow!("elf2uf2-rs failed: {}", e))?;
        if !status.success() {
            bail!("elf2uf2-rs conversion failed");
        }
    }

    if !args.dry_run {
        write_identity_manifest(&root, &elf)?;
        println!("==> pico build: firmware identity manifest written");
    }

    println!("==> pico build: done — {}", uf2.display());
    Ok(())
}

fn write_identity_manifest(root: &Path, elf: &Path) -> Result<()> {
    use sha2::{Digest, Sha256};

    let git_rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".into());

    let firmware_sha256 = if elf.exists() {
        let bytes = std::fs::read(elf)?;
        hex::encode(Sha256::digest(&bytes))
    } else {
        "not-built".into()
    };

    let cyw43_assets: Vec<AssetEntry> = CYW43_ASSETS
        .iter()
        .map(|(filename, expected)| AssetEntry {
            filename: filename.to_string(),
            sha256: expected.to_string(),
        })
        .collect();

    let identity = FirmwareIdentity {
        schema: "conduit-pico-w-signal/identity@1".into(),
        git_revision: git_rev,
        target: TARGET.into(),
        profile: PROFILE.into(),
        firmware_sha256,
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets,
    };

    let manifest_path = root
        .join("target")
        .join(TARGET)
        .join(PROFILE)
        .join(format!("{}.identity.json", FIRMWARE_PACKAGE));

    std::fs::create_dir_all(manifest_path.parent().unwrap())?;
    let json = serde_json::to_string_pretty(&identity)?;
    std::fs::write(&manifest_path, json)?;
    println!("  identity manifest: {}", manifest_path.display());
    Ok(())
}

/// Download and verify CYW43 assets from the pinned Embassy commit.
pub fn refresh_radio_assets(dry_run: bool) -> Result<()> {
    let root = repo_root();
    let asset_dir = root.join(CYW43_ASSET_DIR);
    std::fs::create_dir_all(&asset_dir)?;

    for (filename, expected) in CYW43_ASSETS {
        let url = format!(
            "https://raw.githubusercontent.com/embassy-rs/embassy/{}/cyw43-firmware/{}",
            CYW43_COMMIT, filename
        );
        let dest = asset_dir.join(filename);
        println!("==> downloading {} -> {}", url, dest.display());
        if dry_run {
            continue;
        }
        // Use curl or wget
        let status = Command::new("curl")
            .args(["-fsSL", "-o", dest.to_str().unwrap(), &url])
            .status()
            .map_err(|e| anyhow::anyhow!("curl failed: {}", e))?;
        if !status.success() {
            bail!("failed to download {}", url);
        }
        verify_sha256(&dest, expected)?;
        println!("  ✓ verified {}", filename);
    }
    println!("==> CYW43 assets refreshed and verified");
    Ok(())
}

pub fn uf2_path(root: &PathBuf) -> PathBuf {
    root.join("target")
        .join(TARGET)
        .join(PROFILE)
        .join(format!("{}.uf2", FIRMWARE_PACKAGE))
}
