use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::doctor::{
    repo_root, sha256_file, verify_sha256, CYW43_ASSETS, CYW43_ASSET_DIR, CYW43_COMMIT,
};
use super::{PicoArgs, PicoResult};

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

pub fn run_build(args: &PicoArgs) -> PicoResult<()> {
    println!("==> pico build: verifying CYW43 assets");
    let root = repo_root();
    let asset_dir = root.join(CYW43_ASSET_DIR);
    for (filename, expected) in CYW43_ASSETS {
        let path = asset_dir.join(filename);
        if args.dry_run {
            println!("  planned: verify {}", path.display());
        } else {
            verify_sha256(&path, expected)?;
        }
    }

    let manifest = root
        .join("firmware")
        .join(FIRMWARE_PACKAGE)
        .join("Cargo.toml");
    let manifest_text = manifest
        .to_str()
        .ok_or("firmware manifest path is not UTF-8")?;
    let build_args = [
        "build",
        "--locked",
        "--manifest-path",
        manifest_text,
        "--package",
        FIRMWARE_PACKAGE,
        "--target",
        TARGET,
        "--release",
    ];
    println!("==> pico build: cargo {}", build_args.join(" "));
    if !args.dry_run {
        let status = Command::new("cargo").args(build_args).status()?;
        if !status.success() {
            return Err("cargo build for Pico W firmware failed".into());
        }
    }

    let elf = root
        .join("target")
        .join(TARGET)
        .join(PROFILE)
        .join(FIRMWARE_PACKAGE);
    let uf2 = elf.with_extension("uf2");

    println!(
        "==> pico build: elf2uf2-rs {} {}",
        elf.display(),
        uf2.display()
    );
    if !args.dry_run {
        let status = Command::new("elf2uf2-rs").arg(&elf).arg(&uf2).status()?;
        if !status.success() {
            return Err("elf2uf2-rs conversion failed".into());
        }
        write_identity_manifest(&root, &elf)?;
    }

    println!("==> pico build: done — {}", uf2.display());
    Ok(())
}

fn write_identity_manifest(root: &Path, elf: &Path) -> PicoResult<()> {
    let git_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !git_output.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let git_revision = String::from_utf8(git_output.stdout)?.trim().to_owned();
    let firmware_sha256 = sha256_file(elf)?;

    let cyw43_assets = CYW43_ASSETS
        .iter()
        .map(|(filename, expected)| AssetEntry {
            filename: (*filename).to_string(),
            sha256: (*expected).to_string(),
        })
        .collect();

    let identity = FirmwareIdentity {
        schema: "conduit-pico-w-signal/identity@1".into(),
        git_revision,
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
        .join(format!("{FIRMWARE_PACKAGE}.identity.json"));
    std::fs::create_dir_all(
        manifest_path
            .parent()
            .ok_or("identity manifest path has no parent")?,
    )?;
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&identity)?)?;
    println!("  identity manifest: {}", manifest_path.display());
    Ok(())
}

pub fn refresh_radio_assets(dry_run: bool) -> PicoResult<()> {
    let asset_dir = repo_root().join(CYW43_ASSET_DIR);
    if !dry_run {
        std::fs::create_dir_all(&asset_dir)?;
    }

    for (filename, expected) in CYW43_ASSETS {
        let url = format!(
            "https://raw.githubusercontent.com/embassy-rs/embassy/{CYW43_COMMIT}/cyw43-firmware/{filename}"
        );
        let destination = asset_dir.join(filename);
        println!("==> downloading {url} -> {}", destination.display());
        if dry_run {
            continue;
        }
        let status = Command::new("curl")
            .args(["-fL", "--retry", "3", "--retry-delay", "2", "-o"])
            .arg(&destination)
            .arg(&url)
            .status()?;
        if !status.success() {
            return Err(format!("failed to download {url}").into());
        }
        verify_sha256(&destination, expected)?;
    }
    println!("==> CYW43 assets refreshed and verified");
    Ok(())
}

pub fn uf2_path(root: &PathBuf) -> PathBuf {
    root.join("target")
        .join(TARGET)
        .join(PROFILE)
        .join(format!("{FIRMWARE_PACKAGE}.uf2"))
}
