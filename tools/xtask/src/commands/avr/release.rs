use std::{fs, path::Path};

use conduit_host_avr_fabrication::{
    APPLICATION_FLASH_BYTES, ARTIFACT_FORMAT, BOARD, BOOTLOADER, BOOTLOADER_PROTOCOL,
    BOOT_REGION_BYTES, BOOT_REGION_START, BUILDER_ADAPTER, CLOCK_HZ, FLASH_BYTES, FQBN, MCU,
    PACKAGE_ID, PACKAGE_REVISION, RESET_TRANSITION, SPORE_REGION_BYTES, SPORE_REGION_START,
    SRAM_BYTES, TARGET_ID,
};
use serde::Serialize;

use crate::{cli::GlobalOpts, workspace::workspace_root};

use super::{run_build, write_receipt};

const ARTIFACT_NAME: &str = "promicro-atmega32u4-5v-16mhz.hex";
const MANIFEST_NAME: &str = "avr-promicro-atmega32u4-5v-16mhz.json";

#[derive(Serialize)]
struct ReleaseManifest {
    schema: &'static str,
    target_id: &'static str,
    image_id: String,
    source_identity: String,
    fabrication_package_id: &'static str,
    fabrication_package_revision: u32,
    output: &'static str,
    builder_adapter: &'static str,
    deployment_adapter: Option<&'static str>,
    board: BoardManifest,
    flash: FlashManifest,
    artifact: ArtifactManifest,
    bootloader: BootloaderManifest,
    join: JoinManifest,
}

#[derive(Serialize)]
struct BoardManifest {
    model: &'static str,
    fqbn: &'static str,
    mcu: &'static str,
    clock_hz: u64,
    voltage_mv: u64,
    sram_bytes: u64,
}

#[derive(Serialize)]
struct FlashManifest {
    total_bytes: u64,
    application_bytes: u64,
    boot_region_start: u64,
    boot_region_bytes: u64,
    compiled_application_bytes: u64,
    spore_region_start: u64,
    spore_region_bytes: u64,
}

#[derive(Serialize)]
struct ArtifactManifest {
    path: &'static str,
    media_type: &'static str,
    bytes: u64,
    sha256: String,
    format: &'static str,
}

#[derive(Serialize)]
struct BootloaderManifest {
    name: &'static str,
    protocol: &'static str,
    reset_transition: &'static str,
    browser_deployment_implemented: bool,
}

#[derive(Serialize)]
struct JoinManifest {
    contract: &'static str,
    behavior: &'static str,
    automated_observation_implemented: bool,
    authenticated_join_implemented: bool,
}

pub(super) fn run(output: &Path, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    let built = run_build(
        Path::new("target/avr-promicro/release-build-receipt.json"),
        opts,
    )?;
    if opts.dry_run {
        if !opts.quiet {
            println!("would seal exact Pro Micro release in {}", output.display());
        }
        return Ok(());
    }
    let root = workspace_root()?;
    let output = root.join(output);
    fs::create_dir_all(&output)?;
    let artifact = output.join(ARTIFACT_NAME);
    fs::copy(&built.path, &artifact)?;
    let artifact_bytes = fs::metadata(&artifact)?.len();
    let manifest = ReleaseManifest {
        schema: "conduit.release/avr-intel-hex@1",
        target_id: TARGET_ID,
        image_id: format!("conduit-release/avr-promicro@{}", built.artifact_sha256),
        source_identity: format!("git:{}", built.identity.source_sha),
        fabrication_package_id: PACKAGE_ID,
        fabrication_package_revision: PACKAGE_REVISION,
        output: ARTIFACT_FORMAT,
        builder_adapter: BUILDER_ADAPTER,
        deployment_adapter: None,
        board: BoardManifest {
            model: BOARD,
            fqbn: FQBN,
            mcu: MCU,
            clock_hz: CLOCK_HZ,
            voltage_mv: 5_000,
            sram_bytes: SRAM_BYTES,
        },
        flash: FlashManifest {
            total_bytes: FLASH_BYTES,
            application_bytes: APPLICATION_FLASH_BYTES,
            boot_region_start: BOOT_REGION_START,
            boot_region_bytes: BOOT_REGION_BYTES,
            compiled_application_bytes: built.flash_bytes,
            spore_region_start: SPORE_REGION_START,
            spore_region_bytes: SPORE_REGION_BYTES,
        },
        artifact: ArtifactManifest {
            path: ARTIFACT_NAME,
            media_type: "application/vnd.conduit.intel-hex",
            bytes: artifact_bytes,
            sha256: format!("sha256:{}", built.artifact_sha256),
            format: ARTIFACT_FORMAT,
        },
        bootloader: BootloaderManifest {
            name: BOOTLOADER,
            protocol: BOOTLOADER_PROTOCOL,
            reset_transition: RESET_TRANSITION,
            browser_deployment_implemented: false,
        },
        join: JoinManifest {
            contract: "conduit.avr/external-cdc-attestation-before-join@1",
            behavior: "external programmer, fresh Boot, exact ATTEST observation, then explicit authenticated admission",
            automated_observation_implemented: false,
            authenticated_join_implemented: false,
        },
    };
    write_receipt(&output.join(MANIFEST_NAME), &manifest, opts)?;
    if !opts.quiet {
        println!("AVR generic release: {}", output.display());
    }
    Ok(())
}
