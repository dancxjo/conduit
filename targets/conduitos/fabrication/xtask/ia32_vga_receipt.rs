//! Emulator verification of the attended IA-32 legacy-BIOS VGA receipt.

use std::{
    fs,
    io::Write,
    os::unix::net::UnixStream,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use super::ConduitosError;

pub(super) fn capture_and_validate(
    monitor_path: &Path,
    vga_path: &Path,
    product: &serde_json::Value,
) -> Result<(), ConduitosError> {
    let mut monitor = UnixStream::connect(monitor_path)
        .map_err(|error| refusal("ia32-vga-receipt-unavailable", error))?;
    writeln!(monitor, "pmemsave 0xb8000 0xfa0 \"{}\"", vga_path.display())
        .map_err(|error| refusal("ia32-vga-receipt-unavailable", error))?;
    monitor
        .flush()
        .map_err(|error| refusal("ia32-vga-receipt-unavailable", error))?;

    let deadline = Instant::now() + Duration::from_secs(2);
    let bytes = loop {
        match fs::read(vga_path) {
            Ok(bytes) if bytes.len() == 4000 => break bytes,
            Ok(_) | Err(_) if Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            Ok(bytes) => {
                return Err(refusal(
                    "ia32-vga-receipt-invalid",
                    format!("captured {} bytes instead of 4000", bytes.len()),
                ));
            }
            Err(error) => return Err(refusal("ia32-vga-receipt-unavailable", error)),
        }
    };
    validate_bytes(&bytes, product)
}

fn validate_bytes(bytes: &[u8], product: &serde_json::Value) -> Result<(), ConduitosError> {
    let (rows, remainder) = bytes.as_chunks::<160>();
    if !remainder.is_empty() || rows.len() != 25 {
        return Err(refusal(
            "ia32-vga-receipt-invalid",
            "VGA text receipt must contain exactly 25 complete rows",
        ));
    }
    let lines: Vec<String> = rows
        .iter()
        .map(|row| {
            let text: String = row
                .iter()
                .step_by(2)
                .map(|byte| char::from(*byte))
                .collect();
            text.trim_end().to_owned()
        })
        .collect();
    for required in [
        "CONDUITOS IA-32 / LEGACY BIOS",
        "HELLO, CONDUITOS",
        "PRODUCT ENTRY READY",
        product["profile_id"].as_str().unwrap_or_default(),
        product["build_id"].as_str().unwrap_or_default(),
        product["image_id"].as_str().unwrap_or_default(),
        product["host_id"].as_str().unwrap_or_default(),
        product["boot_id"].as_str().unwrap_or_default(),
    ] {
        if required.is_empty() || required.len() > 80 || !lines.iter().any(|line| line == required)
        {
            return Err(refusal(
                "ia32-vga-receipt-invalid",
                format!("VGA text receipt omitted exact line {required:?}"),
            ));
        }
    }
    Ok(())
}

fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_or_blank_receipts_refuse() {
        let product = serde_json::json!({
            "profile_id": "profile",
            "build_id": "build",
            "image_id": "image",
            "host_id": "host",
            "boot_id": "boot"
        });
        assert!(validate_bytes(&[0; 3999], &product).is_err());
        assert!(validate_bytes(&[0; 4000], &product).is_err());
    }
}
