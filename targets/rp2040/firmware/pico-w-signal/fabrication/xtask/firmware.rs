use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::process::command_for;

use super::appliance_identity::{
    write_appliance_hil_client_identity_manifest, write_appliance_identity_manifest,
};
use super::doctor::{
    repo_root, sha256_file, verify_sha256, CYW43_ASSETS, CYW43_ASSET_DIR, CYW43_COMMIT,
};
use super::{PicoArgs, PicoResult};

pub const FIRMWARE_PACKAGE: &str = "conduit-pico-w-signal";
pub const TARGET: &str = "thumbv6m-none-eabi";
pub const PROFILE: &str = "release";
const MIDI_FIXTURE_BINARY: &str = "conduit-pico-w-midi-fixture";
const PETE_CAPSTONE_BINARY: &str = "conduit-pico-w-pete-capstone";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FirmwareIdentity {
    pub schema: String,
    pub git_revision: String,
    pub target: String,
    pub profile: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub firmware_sha256: String,
    pub generated_image: GeneratedImageIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r1_control_images: Option<R1ControlImageFamily>,
    pub cyw43_commit: String,
    pub cyw43_assets: Vec<AssetEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct R1ControlImageFamily {
    pub plan_a: GeneratedImageIdentity,
    pub plan_b: GeneratedImageIdentity,
    pub plan_c: GeneratedImageIdentity,
}

impl FirmwareIdentity {
    pub fn verified_r1_control_image(
        &self,
        plan_id: &conduit_core::PlanId,
    ) -> PicoResult<&GeneratedImageIdentity> {
        let family = self.verified_r1_control_images()?;
        [&family.plan_a, &family.plan_b, &family.plan_c]
            .into_iter()
            .find(|image| image.plan_id == plan_id.as_str())
            .ok_or_else(|| "R1 control Plan is absent from the firmware image family".into())
    }

    pub fn verified_r1_control_images(&self) -> PicoResult<&R1ControlImageFamily> {
        let family = self
            .r1_control_images
            .as_ref()
            .ok_or("firmware identity has no R1 control image family")?;
        if self.schema != "conduit-pico-w-signal/identity@2"
            || self.firmware_mode != "r1-control"
            || self.generated_image.schema != "conduit.pico-network.generated-image@1"
            || self.generated_image.firmware_mode != self.firmware_mode
            || self.generated_image.firmware_build_id != self.firmware_build_id
        {
            return Err("R1 composite firmware primary network identity is invalid".into());
        }
        for (image, routes) in [
            (
                &family.plan_a,
                conduit_r1_network_conformance::R1SignalRouteSet::WebSocketOnly,
            ),
            (
                &family.plan_b,
                conduit_r1_network_conformance::R1SignalRouteSet::UsbOnly,
            ),
            (
                &family.plan_c,
                conduit_r1_network_conformance::R1SignalRouteSet::WebSocketThenUsb,
            ),
        ] {
            let exact = conduit_r1_network_conformance::exact_r1_control_plan(
                conduit_core::BootId::from(conduit_r1_network_conformance::R1_PICO_BOOT_ID),
                routes,
            )?;
            let fragment = exact
                .plan
                .fragments
                .iter()
                .find(|fragment| {
                    fragment.host_id.as_str() == conduit_r1_network_conformance::R1_PICO_HOST_ID
                })
                .ok_or("exact R1 control Plan has no Pico fragment")?;
            if image.schema != "conduit.pico-signal.generated-image@1"
                || image.firmware_mode != self.firmware_mode
                || image.source_document_id != exact.plan.source_document_id.as_str()
                || image.checked_form_id != exact.plan.checked_form_id.as_str()
                || image.expanded_form_id != exact.plan.expanded_form_id.as_str()
                || image.plan_id != exact.plan.plan_id.as_str()
                || image.fragment_id != fragment.fragment_id.as_str()
                || image.host_id != conduit_r1_network_conformance::R1_PICO_HOST_ID
                || image.boot_id != conduit_r1_network_conformance::R1_PICO_BOOT_ID
                || image.nodes != 1
                || image.cords != 1
                || image.host_operations != 1
                || image.cord_value_slots != 1
                || image.cord_value_bytes != conduit_signal::SIGNAL_ENCODED_LEN
            {
                return Err("R1 composite firmware control image family is invalid".into());
            }
        }
        Ok(family)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeneratedImageIdentity {
    pub schema: String,
    pub firmware_mode: String,
    pub firmware_build_id: String,
    pub source_document_id: String,
    pub checked_form_id: String,
    pub expanded_form_id: String,
    pub plan_id: String,
    pub fragment_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub active_play_id: String,
    pub boot_sign_id: String,
    pub presentation_ids: Vec<String>,
    pub presentation_sign_ids: Vec<String>,
    pub terminal_sign_id: String,
    pub offer_generation: u64,
    pub nodes: usize,
    pub cords: usize,
    pub host_operations: usize,
    pub cord_value_slots: u16,
    pub cord_value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetEntry {
    pub filename: String,
    pub sha256: String,
}

pub fn run_build(args: &PicoArgs) -> PicoResult<()> {
    if args.indicator_resource {
        return super::indicator_build::run(args);
    }
    println!("==> pico build: verifying required assets");
    let root = repo_root();
    if !args.usb_midi_fixture {
        let asset_dir = root.join(CYW43_ASSET_DIR);
        for (filename, expected) in CYW43_ASSETS {
            let path = asset_dir.join(filename);
            if args.dry_run {
                println!("  planned: verify {}", path.display());
            } else {
                verify_sha256(&path, expected)?;
            }
        }
    }

    let manifest = firmware_root(&root).join("Cargo.toml");
    let manifest_text = manifest
        .to_str()
        .ok_or("firmware manifest path is not UTF-8")?;
    let mut build_args = vec![
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
    if args.pete_capstone {
        build_args.extend([
            "--bin",
            PETE_CAPSTONE_BINARY,
            "--no-default-features",
            "--features",
            "pete-capstone",
        ]);
    } else if args.usb_midi_fixture {
        build_args.extend([
            "--bin",
            MIDI_FIXTURE_BINARY,
            "--no-default-features",
            "--features",
            "usb-midi-fixture",
        ]);
    } else if args.distributed_lenia {
        build_args.extend(["--no-default-features", "--features", "distributed-lenia"]);
    } else if args.bluetooth_line {
        build_args.extend(["--no-default-features", "--features", "bluetooth-line"]);
    } else if args.appliance_hello {
        build_args.extend(["--no-default-features", "--features", "appliance-hello"]);
    } else if args.appliance_hil_client {
        build_args.extend([
            "--no-default-features",
            "--features",
            "appliance-hil-client",
        ]);
    } else if args.r1_control {
        build_args.extend(["--no-default-features", "--features", "r1-control"]);
    } else if args.wifi_bootstrap {
        build_args.extend(["--no-default-features", "--features", "wifi-bootstrap"]);
    } else if args.triple_remote {
        build_args.extend(["--no-default-features", "--features", "triple-remote"]);
    } else if args.usb_remote {
        build_args.extend(["--no-default-features", "--features", "usb-remote"]);
    }
    println!("==> pico build: cargo {}", build_args.join(" "));
    let generated_identity_sidecar = generated_identity_sidecar_path(&root);
    let control_identity_sidecars = control_identity_sidecar_paths(&root);
    let appliance_identity_sidecar = appliance_identity_sidecar_path(&root);
    let appliance_hil_client_identity_sidecar = appliance_hil_client_identity_sidecar_path(&root);
    if args.dry_run {
        let planned_sidecar = if args.usb_midi_fixture || args.pete_capstone {
            None
        } else if args.appliance_hello {
            Some(&appliance_identity_sidecar)
        } else if args.appliance_hil_client {
            Some(&appliance_hil_client_identity_sidecar)
        } else {
            Some(&generated_identity_sidecar)
        };
        if let Some(planned_sidecar) = planned_sidecar {
            println!("  planned: identity sidecar {}", planned_sidecar.display());
        }
    } else {
        if generated_identity_sidecar.exists() {
            std::fs::remove_file(&generated_identity_sidecar)?;
        }
        if appliance_identity_sidecar.exists() {
            std::fs::remove_file(&appliance_identity_sidecar)?;
        }
        if appliance_hil_client_identity_sidecar.exists() {
            std::fs::remove_file(&appliance_hil_client_identity_sidecar)?;
        }
        for sidecar in &control_identity_sidecars {
            if sidecar.exists() {
                std::fs::remove_file(sidecar)?;
            }
        }
        let status = Command::new("cargo")
            .args(build_args)
            .env(
                "CONDUIT_PICO_SIGNAL_IDENTITY_SIDECAR",
                &generated_identity_sidecar,
            )
            .env("CONDUIT_PICO_SIGNAL_IDENTITY_RERUN", build_rerun_nonce())
            .env(
                "CONDUIT_R1_CONTROL_PLAN_A_IDENTITY_SIDECAR",
                &control_identity_sidecars[0],
            )
            .env(
                "CONDUIT_R1_CONTROL_PLAN_B_IDENTITY_SIDECAR",
                &control_identity_sidecars[1],
            )
            .env(
                "CONDUIT_R1_CONTROL_PLAN_C_IDENTITY_SIDECAR",
                &control_identity_sidecars[2],
            )
            .env(
                "CONDUIT_PICO_APPLIANCE_IDENTITY_SIDECAR",
                &appliance_identity_sidecar,
            )
            .env(
                "CONDUIT_PICO_APPLIANCE_HIL_CLIENT_IDENTITY_SIDECAR",
                &appliance_hil_client_identity_sidecar,
            )
            .status()?;
        if !status.success() {
            return Err("cargo build for Pico W firmware failed".into());
        }
    }

    let elf = if args.pete_capstone {
        firmware_target_profile_dir(&root).join(PETE_CAPSTONE_BINARY)
    } else if args.usb_midi_fixture {
        firmware_target_profile_dir(&root).join(MIDI_FIXTURE_BINARY)
    } else {
        firmware_elf_path(&root)
    };
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
        if args.pete_capstone {
            let revision = git_revision(&root)?;
            let tree_state = git_tree_state(&root)?;
            let identity = serde_json::json!({
                "schema": "conduit.pete/capstone-image@1",
                "git_revision": revision,
                "firmware_build_id": format!(
                    "conduit-pico-w-pete-capstone:{revision}:{tree_state}:{TARGET}:{PROFILE}:physical-play@1"
                ),
                "target": TARGET,
                "profile": PROFILE,
                "firmware_mode": "pete-capstone",
                "firmware_sha256": sha256_file(&elf)?,
                "usb_serial": "pete-capstone",
                "translator_oe": {"gpio": 19, "level": "high"},
                "power_toggle": {"gpio": 18, "level": "low"},
                "create_uart": "supervised_57600_8n1",
                "robot_control_capable": true,
                "form": "pete-capstone",
                "kernel": "conduit-kernel",
                "oi_exposed": false,
            });
            std::fs::write(
                identity_manifest_path(&root),
                serde_json::to_string_pretty(&identity)?,
            )?;
        } else if args.usb_midi_fixture {
            println!("  fixture artifact: build-only; no Conduit Plan identity is claimed");
        } else if args.appliance_hello {
            write_appliance_identity_manifest(&root, &elf, &appliance_identity_sidecar)?;
        } else if args.appliance_hil_client {
            write_appliance_hil_client_identity_manifest(
                &root,
                &elf,
                &appliance_hil_client_identity_sidecar,
            )?;
        } else {
            write_identity_manifest(
                &root,
                &elf,
                &generated_identity_sidecar,
                &control_identity_sidecars,
            )?;
        }
    }

    println!("==> pico build: done — {}", uf2.display());
    Ok(())
}

fn git_revision(root: &Path) -> PicoResult<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()?;
    if !output.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

fn git_tree_state(root: &Path) -> PicoResult<&'static str> {
    let status = Command::new("git")
        .args(["diff", "--quiet", "--ignore-submodules", "--"])
        .current_dir(root)
        .status()?;
    Ok(if status.success() { "clean" } else { "dirty" })
}

fn write_identity_manifest(
    root: &Path,
    elf: &Path,
    generated_identity_sidecar: &Path,
    control_identity_sidecars: &[PathBuf; 3],
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
    let r1_control_images = if generated_image.firmware_mode == "r1-control" {
        Some(R1ControlImageFamily {
            plan_a: read_generated_image_identity(&control_identity_sidecars[0])?,
            plan_b: read_generated_image_identity(&control_identity_sidecars[1])?,
            plan_c: read_generated_image_identity(&control_identity_sidecars[2])?,
        })
    } else {
        None
    };

    let cyw43_assets = CYW43_ASSETS
        .iter()
        .map(|(filename, expected)| AssetEntry {
            filename: (*filename).to_string(),
            sha256: (*expected).to_string(),
        })
        .collect();

    let identity = FirmwareIdentity {
        schema: if r1_control_images.is_some() {
            "conduit-pico-w-signal/identity@2"
        } else {
            "conduit-pico-w-signal/identity@1"
        }
        .into(),
        git_revision,
        target: TARGET.into(),
        profile: PROFILE.into(),
        firmware_mode: generated_image.firmware_mode.clone(),
        firmware_build_id: generated_image.firmware_build_id.clone(),
        firmware_sha256,
        generated_image,
        r1_control_images,
        cyw43_commit: CYW43_COMMIT.into(),
        cyw43_assets,
    };
    if identity.r1_control_images.is_some() {
        identity.verified_r1_control_images()?;
    }

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

fn control_identity_sidecar_paths(root: &Path) -> [PathBuf; 3] {
    let directory = firmware_target_profile_dir(root);
    [
        directory.join("r1-control-plan-a.generated-image.json"),
        directory.join("r1-control-plan-b.generated-image.json"),
        directory.join("r1-control-plan-c.generated-image.json"),
    ]
}

pub fn read_identity_manifest(root: &Path) -> PicoResult<FirmwareIdentity> {
    let manifest_path = identity_manifest_path(root);
    let text = std::fs::read_to_string(&manifest_path).map_err(|error| {
        format!(
            "failed to read Pico identity manifest at {}: {error}; run `cargo xtask pico build` first",
            manifest_path.display()
        )
    })?;
    Ok(serde_json::from_str(&text)?)
}

pub fn read_firmware_mode(root: &Path) -> PicoResult<String> {
    let value: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(identity_manifest_path(root))?)?;
    value["firmware_mode"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "firmware identity has no firmware_mode".into())
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
        println!("==> downloading {url} > {}", destination.display());
        if dry_run {
            continue;
        }
        let status = Command::new("curl")
            .args([
                "--fail",
                "--location",
                "--remove-on-error",
                "--retry",
                "3",
                "--retry-all-errors",
                "--retry-delay",
                "2",
                "--output",
            ])
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

pub fn pete_capstone_uf2_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root)
        .join(PETE_CAPSTONE_BINARY)
        .with_extension("uf2")
}

fn firmware_elf_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join(FIRMWARE_PACKAGE)
}

fn generated_identity_sidecar_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join(format!("{FIRMWARE_PACKAGE}.generated-image.json"))
}

fn appliance_identity_sidecar_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join("pico-appliance.generated-image.json")
}

fn appliance_hil_client_identity_sidecar_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join("pico-appliance-hil-client.generated-image.json")
}

pub(super) fn identity_manifest_path(root: &Path) -> PathBuf {
    firmware_target_profile_dir(root).join(format!("{FIRMWARE_PACKAGE}.identity.json"))
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
    root.join("targets/rp2040/firmware/pico-w-signal")
}
