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

/// Only a newline-terminated guest completion can authorize a single capture.
pub(super) fn completed_boot(transcript: &str) -> Option<&str> {
    transcript.split_inclusive('\n').find_map(|line| {
        line.strip_suffix('\n')?
            .strip_prefix("CONDUIT_IA32_VGA_RECEIPT_WRITTEN ")
            .filter(|boot| !boot.is_empty())
    })
}

pub(super) fn validate_completion(
    transcript: &str,
    product: &serde_json::Value,
) -> Result<(), ConduitosError> {
    match (completed_boot(transcript), product["boot_id"].as_str()) {
        (Some(completed), Some(expected)) if completed == expected => Ok(()),
        _ => Err(refusal(
            "ia32-vga-receipt-incomplete",
            "guest VGA completion lacks the exact boot identity",
        )),
    }
}

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
    // The raw .bin remains useful locally; CI diagnostics retain .json files.
    // Save the original captured bytes before validation, including failed rows.
    fs::write(
        vga_path.with_extension("json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema": "conduit.conduitos/ia32-vga-capture@1",
            "boot_id": product["boot_id"],
            "bytes": bytes,
        }))
        .map_err(|error| refusal("ia32-vga-receipt-unavailable", error))?,
    )
    .map_err(|error| refusal("ia32-vga-receipt-unavailable", error))?;
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
    fn capture_waits_for_complete_exact_guest_boot_acknowledgement() {
        let product = serde_json::json!({"boot_id": "boot-123"});
        for transcript in [
            "CONDUIT_IA32_PRODUCT {}\nCONDUIT_OBSERVATORY_SNAPSHOT {}\n",
            "CONDUIT_IA32_VGA_RECEIPT_WRITTEN boot-123",
            "CONDUIT_IA32_VGA_RECEIPT_WRITTEN stale-boot\n",
            "CONDUIT_IA32_VGA_RECEIPT_WRITTEN \n",
        ] {
            assert!(validate_completion(transcript, &product).is_err());
        }
        assert!(
            validate_completion("CONDUIT_IA32_VGA_RECEIPT_WRITTEN boot-123\n", &product).is_ok()
        );
    }

    #[test]
    fn complete_receipt_requires_final_ready_row_and_exact_identity() {
        let product = serde_json::json!({
            "profile_id": "profile", "build_id": "build", "image_id": "image",
            "host_id": "host", "boot_id": "boot"
        });
        let mut bytes = [b' '; 4000];
        for (row, line) in [
            (1, "CONDUITOS IA-32 / LEGACY BIOS"),
            (3, "HELLO, CONDUITOS"),
            (6, "profile"),
            (9, "build"),
            (12, "image"),
            (15, "host"),
            (18, "boot"),
            (20, "PRODUCT ENTRY READY"),
        ] {
            for (column, byte) in line.bytes().enumerate() {
                bytes[row * 160 + column * 2] = byte;
            }
        }
        assert!(validate_bytes(&bytes, &product).is_ok());
        let mut stale = product.clone();
        stale["boot_id"] = "another-boot".into();
        assert!(validate_bytes(&bytes, &stale).is_err());
        bytes[20 * 160..21 * 160].fill(b' ');
        assert!(validate_bytes(&bytes, &product).is_err());
    }

    #[test]
    fn failed_capture_retains_original_bytes_in_uploaded_diagnostics() {
        use std::io::{BufRead, BufReader};
        use std::os::unix::net::UnixListener;
        let directory = std::env::temp_dir().join(format!(
            "conduit-vga-capture-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&directory).unwrap();
        let socket = directory.join("monitor.sock");
        let capture = directory.join("vga.bin");
        let listener = UnixListener::bind(&socket).unwrap();
        let destination = capture.clone();
        let writer = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut command = String::new();
            BufReader::new(stream).read_line(&mut command).unwrap();
            assert!(command.starts_with("pmemsave 0xb8000 0xfa0 "));
            fs::write(destination, [0; 4000]).unwrap();
        });
        let product = serde_json::json!({"boot_id": "boot-123"});
        assert!(capture_and_validate(&socket, &capture, &product).is_err());
        writer.join().unwrap();
        let retained: serde_json::Value =
            serde_json::from_slice(&fs::read(capture.with_extension("json")).unwrap()).unwrap();
        assert_eq!(retained["boot_id"], "boot-123");
        assert_eq!(retained["bytes"], serde_json::json!([0; 4000].as_slice()));
        assert_eq!(fs::read(capture).unwrap(), [0; 4000]);
        fs::remove_dir_all(directory).unwrap();
    }

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
