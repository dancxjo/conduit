use std::{fs, path::Path, process::Command};

use conduit_host_esp32_fabrication::{Esp32FamilyTarget, NATIVE_SPORE_REGION_START};
use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{provision_espflash, require_success, sha256_file, write_receipt};

#[derive(Serialize)]
struct BrowserReleaseManifest {
    schema: &'static str,
    target_id: String,
    image_id: String,
    source_identity: String,
    artifact_layout: BrowserArtifactLayout,
    artifact_sha256: String,
    bytes: u64,
    segments: Vec<BrowserArtifactSegment>,
}

#[derive(Serialize)]
struct BrowserArtifactLayout {
    format: &'static str,
    flash_offset: u32,
}

#[derive(Serialize)]
struct BrowserArtifactSegment {
    offset: u32,
    path: String,
    bytes: u64,
}

pub(super) fn write(
    root: &Path,
    target: Esp32FamilyTarget,
    elf: &Path,
    output: &Path,
    source_sha: &str,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = root.join(output);
    let parent = output
        .parent()
        .ok_or("browser ESP32 artifact output has no parent directory")?;
    fs::create_dir_all(parent)?;
    let tool = provision_espflash(root)?;
    let mut command = Command::new(&tool);
    command
        .args([
            "save-image",
            "--chip",
            target.facts().espflash_chip,
            "--merge",
            "--skip-padding",
        ])
        .arg(elf)
        .arg(&output);
    require_success(command, "ESP32 generic browser release packaging")?;
    let bytes = fs::metadata(&output)?.len();
    if bytes == 0 || bytes > NATIVE_SPORE_REGION_START {
        return Err(format!(
            "ESP32 generic browser release {} overlaps the reserved native Spore sector",
            output.display()
        )
        .into());
    }
    let filename = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("browser ESP32 artifact filename is not UTF-8")?;
    let facts = target.facts();
    let manifest = BrowserReleaseManifest {
        schema: "conduit.release/target-artifact@1",
        target_id: format!("esp32/{}/{}", facts.architecture, facts.machine),
        image_id: format!(
            "conduit-release/esp32-{}-signal/{source_sha}",
            facts.selector
        ),
        source_identity: format!("git:{source_sha}"),
        artifact_layout: BrowserArtifactLayout {
            format: "espressif-merged-image",
            flash_offset: 0,
        },
        artifact_sha256: format!("sha256:{}", sha256_file(&output)?),
        bytes,
        segments: vec![BrowserArtifactSegment {
            offset: 0,
            path: format!("./{filename}"),
            bytes,
        }],
    };
    write_receipt(&output.with_extension("json"), &manifest, opts)
}
