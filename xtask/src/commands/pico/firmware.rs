use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::process::command_for;

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
    pub firmware_build_id: String,
    pub firmware_sha256: String,
    pub generated_image: GeneratedImageIdentity,
    pub cyw43_commit: String,
    pub cyw43_assets: Vec<AssetEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedImageIdentity {
    pub schema: String,
    pub firmware_build_id: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub plan_id: String,
    pub fragment_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub active_play_id: String,
    pub boot_evidence_id: String,
    pub presentation_ids: Vec<String>,
    pub presentation_evidence_ids: Vec<String>,
    pub terminal_evidence_id: String,
    pub offer_generation: u64,
    pub nodes: usize,
    pub cords: usize,
    pub host_operations: usize,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub evidence_items: u16,
    pub evidence_bytes: u32,
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

    let manifest = firmware_root(&root).join("Cargo.toml");
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
    let generated_identity_sidecar = generated_identity_sidecar_path(&root);
    if args.dry_run {
        println!(
            "  planned: generated identity sidecar {}",
            generated_identity_sidecar.display()
        );
    } else {
        if generated_identity_sidecar.exists() {
            std::fs::remove_file(&generated_identity_sidecar)?;
        }
        let status = Command::new("cargo")
            .args(build_args)
            .env(
                "CONDUIT_PICO_SIGNAL_IDENTITY_SIDECAR",
                &generated_identity_sidecar,
            )
            .env("CONDUIT_PICO_SIGNAL_IDENTITY_RERUN", build_rerun_nonce())
            .status()?;
        if !status.success() {
            return Err("cargo build for Pico W firmware failed".into());
        }
    }

    let elf = firmware_elf_path(&root);
    let uf2 = elf.with_extension("uf2");

    println!(
        "==> pico build: elf2uf2-rs {} {}",
        elf.display(),
        uf2.display()
    );
    if !args.dry_run {
        if !elf.exists() {
            return Err(format!(
                "Pico firmware ELF not found at {}; cargo built an unexpected artifact path",
                elf.display()
            )
            .into());
        }
        let status = command_for("elf2uf2-rs").arg(&elf).arg(&uf2).status()?;
        if !status.success() {
            return Err("elf2uf2-rs conversion failed".into());
        }
        write_identity_manifest(&root, &elf, &generated_identity_sidecar)?;
    }

    println!("==> pico build: done — {}", uf2.display());
    Ok(())
}

fn write_identity_manifest(
    root: &Path,
    elf: &Path,
    generated_identity_sidecar: &Path,
) -> PicoResult<()> {
    let git_output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !git_output.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let git_revision = String::from_utf8(git_output.stdout)?.trim().to_owned();
    let firmware_sha256 = sha256_file(elf)?;
    let generated_image = read_generated_image_identity(generated_identity_sidecar)?;

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
        firmware_build_id: generated_image.firmware_build_id.clone(),
        firmware_sha256,
        generated_image,
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets,
    };

    let manifest_path =
        firmware_target_profile_dir(root).join(format!("{FIRMWARE_PACKAGE}.identity.json"));
    std::fs::create_dir_all(
        manifest_path
            .parent()
            .ok_or("identity manifest path has no parent")?,
    )?;
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&identity)?)?;
    println!("  identity manifest: {}", manifest_path.display());
    Ok(())
}

pub fn read_identity_manifest(root: &Path) -> PicoResult<FirmwareIdentity> {
    let manifest_path =
        firmware_target_profile_dir(root).join(format!("{FIRMWARE_PACKAGE}.identity.json"));
    let text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "failed to read Pico identity manifest at {}: {error}; run `cargo xtask pico build` first",
            manifest_path.display()
        )
    })?;
    Ok(serde_json::from_str(&text)?)
}

fn read_generated_image_identity(sidecar: &Path) -> PicoResult<GeneratedImageIdentity> {
    let text = std::fs::read_to_string(sidecar).map_err(|error| {
        format!(
            "failed to read generated Pico Signal identity sidecar at {}: {error}",
            sidecar.display()
        )
    })?;
    Ok(serde_json::from_str(&text)?)
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

pub fn uf2_path(root: &Path) -> PathBuf {
    firmware_elf_path(root).with_extension("uf2")
}

fn firmware_elf_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join(FIRMWARE_PACKAGE)
}

fn generated_identity_sidecar_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join(format!("{FIRMWARE_PACKAGE}.generated-image.json"))
}

fn build_rerun_nonce() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().to_string())
        .unwrap_or_else(|_| "clock-before-unix-epoch".to_owned())
}

fn firmware_target_profile_dir(root: &Path) -> PathBuf {
    firmware_root(root)
        .join("target")
        .join(TARGET)
        .join(PROFILE)
}

fn firmware_root(root: &Path) -> PathBuf {
    root.join("firmware").join(FIRMWARE_PACKAGE)
}
