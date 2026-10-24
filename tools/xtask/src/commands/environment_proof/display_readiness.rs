//! Source-pinned attended readiness experiment, distinct from an output Play.
use super::super::owner_preparation::provision_flash_tool;
use super::execute;
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::Path, process::Command};
pub(super) fn flash_and_capture(root: &Path) -> Result<(), Box<dyn Error>> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let output = root.join(format!("target/environment-display-readiness/{stamp}"));
    fs::create_dir_all(&output)?;
    let elf = output.join("root.elf");
    fs::copy(
        root.join(
            "target/environment-esp32/xtensa-esp32-none-elf/release/conduit-esp32-tm1637-probe",
        ),
        &elf,
    )?;
    let source = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()?;
    let dirty = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain"])
        .output()?;
    if !source.status.success() || !dirty.status.success() {
        return Err("source identity unavailable".into());
    }
    let receipt = serde_json::json!({
        "schema":"conduit.environment/display-readiness-image@1", "source_head":String::from_utf8(source.stdout)?.trim(),
        "source_dirty":!dirty.stdout.is_empty(), "artifact":elf,
        "elf_sha256":format!("{:x}", Sha256::digest(fs::read(&elf)?)),
        "profile":"display-ready-root", "sampling_prepared":false,
        "body_started":false, "physical_display_delivery":false,
    });
    fs::write(
        output.join("image.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    let tool = provision_flash_tool(root)?;
    execute(
        Command::new(tool)
            .args([
                "flash",
                "--chip",
                "esp32",
                "--port",
                "/dev/ttyUSB0",
                "--baud",
                "115200",
                "--non-interactive",
                "--skip-update-check",
            ])
            .arg(&elf),
    )?;
    execute(
        Command::new("python3")
            .current_dir(root)
            .arg("bodies/environment/tools/display-readiness.py")
            .arg("--output")
            .arg(output),
    )
}
