use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;

use super::doctor::repo_root;
#[cfg(test)]
use super::firmware::GeneratedImageIdentity;
use super::firmware::{read_identity_manifest, FirmwareIdentity};
use super::{PicoArgs, PicoResult};

const EXPECTED_RECEIPTS: usize = 16;
const DEVICE_ID_NEEDLE: &str = "conduit-pico-w-signal";

pub fn run_verify(args: &PicoArgs) -> PicoResult<()> {
    if args.dry_run {
        println!("==> pico verify (dry-run)");
        println!(
            "  serial port: {}",
            args.port
                .as_deref()
                .unwrap_or("<auto-discover conduit-pico-w-signal>")
        );
        return Ok(());
    }

    let port = resolve_port(args)?;
    println!("==> pico verify: reading receipts from {}", port.display());

    let _ = Command::new("stty")
        .args([
            "-F",
            port.to_str().ok_or("serial path is not UTF-8")?,
            "115200",
            "cs8",
            "-cstopb",
            "-parenb",
            "raw",
            "-echo",
        ])
        .status();

    let identity = read_identity_manifest(&repo_root())?;
    let file = std::fs::OpenOptions::new().read(true).open(&port)?;
    verify_receipts(BufReader::new(file), &identity)
}

fn verify_receipts(reader: impl BufRead, identity: &FirmwareIdentity) -> PicoResult<()> {
    validate_expected_identity(identity)?;
    let mut boot_seen = false;
    let mut receipts = 0usize;
    let mut terminal_seen = false;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_str(line)
            .map_err(|error| format!("malformed receipt JSON: {error}; line: {line}"))?;
        let schema = record["schema"].as_str().unwrap_or_default();

        if schema.starts_with("conduit-pico-w-signal/boot") {
            if boot_seen {
                return Err("duplicate boot identity record received".into());
            }
            if receipts > 0 {
                return Err("boot identity record arrived after presentation receipts".into());
            }
            verify_boot_identity(&record, identity)?;
            boot_seen = true;
            continue;
        }

        if schema.starts_with("conduit-pico-w-signal/terminal") {
            if !boot_seen {
                return Err("terminal record arrived before boot identity".into());
            }
            if record["success"].as_bool() != Some(true) {
                return Err(format!("firmware reported terminal failure: {line}").into());
            }
            verify_terminal_identity(&record, identity)?;
            terminal_seen = true;
            break;
        }

        if schema.starts_with("conduit-pico-w-signal/receipt") {
            if !boot_seen {
                return Err("presentation receipt arrived before boot identity".into());
            }
            let sequence = record["sequence"]
                .as_u64()
                .ok_or("receipt missing sequence")?;
            let expected_sequence = receipts as u64;
            if sequence != expected_sequence {
                return Err(format!(
                    "out-of-order receipt: expected sequence {expected_sequence}, got {sequence}"
                )
                .into());
            }
            verify_presentation_identity(&record, receipts, identity)?;
            receipts += 1;
            continue;
        }

        return Err(format!("unexpected Pico receipt schema: {schema}").into());
    }

    if !boot_seen {
        return Err("no boot identity record received".into());
    }
    if receipts != EXPECTED_RECEIPTS {
        return Err(format!("expected {EXPECTED_RECEIPTS} receipts, got {receipts}").into());
    }
    if !terminal_seen {
        return Err("no successful terminal completion record received".into());
    }

    println!("==> pico verify: all {EXPECTED_RECEIPTS} receipts valid");
    Ok(())
}

fn validate_expected_identity(identity: &FirmwareIdentity) -> PicoResult<()> {
    let expected = &identity.generated_image;
    if identity.firmware_build_id != expected.firmware_build_id {
        return Err(format!(
            "identity manifest firmware_build_id mismatch: top-level {}, generated image {}",
            identity.firmware_build_id, expected.firmware_build_id
        )
        .into());
    }
    if expected.presentation_ids.len() != EXPECTED_RECEIPTS {
        return Err(format!(
            "identity manifest contains {} presentation IDs; expected {EXPECTED_RECEIPTS}",
            expected.presentation_ids.len()
        )
        .into());
    }
    if expected.presentation_evidence_ids.len() != EXPECTED_RECEIPTS {
        return Err(format!(
            "identity manifest contains {} presentation evidence IDs; expected {EXPECTED_RECEIPTS}",
            expected.presentation_evidence_ids.len()
        )
        .into());
    }
    Ok(())
}

fn verify_boot_identity(record: &serde_json::Value, identity: &FirmwareIdentity) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, false)?;
    verify_field(record, "evidence_id", &expected.boot_evidence_id)
}

fn verify_presentation_identity(
    record: &serde_json::Value,
    sequence: usize,
    identity: &FirmwareIdentity,
) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, true)?;
    let expected_level = sequence % 2 == 1;
    let level = record["level"]
        .as_bool()
        .ok_or_else(|| format!("receipt {sequence} missing level"))?;
    if level != expected_level {
        return Err(format!(
            "receipt {sequence}: expected level={expected_level}, got level={level}"
        )
        .into());
    }
    verify_field(
        record,
        "presentation_id",
        &expected.presentation_ids[sequence],
    )?;
    verify_field(
        record,
        "evidence_id",
        &expected.presentation_evidence_ids[sequence],
    )
}

fn verify_terminal_identity(
    record: &serde_json::Value,
    identity: &FirmwareIdentity,
) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, true)?;
    verify_field(record, "evidence_id", &expected.terminal_evidence_id)
}

fn verify_static_identity(
    record: &serde_json::Value,
    identity: &FirmwareIdentity,
    require_active_play: bool,
) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_field(record, "firmware_build_id", &identity.firmware_build_id)?;
    verify_field(record, "source_document_id", &expected.source_document_id)?;
    verify_field(record, "checked_form_id", &expected.checked_form_id)?;
    verify_field(record, "expanded_form_id", &expected.expanded_form_id)?;
    verify_field(record, "plan_id", &expected.plan_id)?;
    verify_field(record, "fragment_id", &expected.fragment_id)?;
    verify_field(record, "host_id", &expected.host_id)?;
    verify_field(record, "boot_id", &expected.boot_id)?;
    if require_active_play {
        verify_field(record, "active_play_id", &expected.active_play_id)?;
    }
    Ok(())
}

fn verify_field(record: &serde_json::Value, field: &str, expected: &str) -> PicoResult<()> {
    let actual = record[field]
        .as_str()
        .ok_or_else(|| format!("receipt missing identity field `{field}`"))?;
    if actual != expected {
        return Err(format!(
            "receipt identity field `{field}` mismatch: expected {expected}, got {actual}"
        )
        .into());
    }
    Ok(())
}

fn resolve_port(args: &PicoArgs) -> PicoResult<PathBuf> {
    if let Some(port) = &args.port {
        let path = PathBuf::from(port);
        if path.exists() {
            return Ok(path);
        }
        return Err(format!("serial port does not exist: {}", path.display()).into());
    }

    let by_id = PathBuf::from("/dev/serial/by-id");
    if by_id.is_dir() {
        let matches = std::fs::read_dir(&by_id)?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(DEVICE_ID_NEEDLE)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        return match matches.len() {
            1 => Ok(matches[0].clone()),
            0 => Err(
                "no Conduit Pico W serial device found; pass --port after connecting the board"
                    .into(),
            ),
            count => Err(format!(
                "{count} matching serial devices found under {}; pass --port",
                by_id.display()
            )
            .into()),
        };
    }

    Err("/dev/serial/by-id is unavailable; pass --port explicitly".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn expected_identity() -> GeneratedImageIdentity {
        GeneratedImageIdentity {
            schema: "conduit.pico-signal.generated-image@1".into(),
            firmware_build_id: "firmware-build".into(),
            source_document_id: "source".into(),
            checked_form_id: "checked".into(),
            expanded_form_id: "expanded".into(),
            plan_id: "plan".into(),
            fragment_id: "fragment".into(),
            host_id: "host".into(),
            boot_id: "boot".into(),
            active_play_id: "play".into(),
            boot_evidence_id: "boot-evidence".into(),
            presentation_ids: (0..EXPECTED_RECEIPTS)
                .map(|sequence| format!("presentation-{sequence}"))
                .collect(),
            presentation_evidence_ids: (0..EXPECTED_RECEIPTS)
                .map(|sequence| format!("evidence-{sequence}"))
                .collect(),
            terminal_evidence_id: "terminal-evidence".into(),
            offer_generation: 1,
            nodes: 2,
            cords: 1,
            host_operations: 2,
            cord_value_slots: 1,
            cord_value_bytes: 9,
            evidence_items: 7,
            evidence_bytes: 327,
        }
    }

    fn expected_firmware_identity() -> FirmwareIdentity {
        FirmwareIdentity {
            schema: "conduit-pico-w-signal/identity@1".into(),
            git_revision: "revision".into(),
            target: "thumbv6m-none-eabi".into(),
            profile: "release".into(),
            firmware_build_id: "firmware-build".into(),
            firmware_sha256: "sha256".into(),
            generated_image: expected_identity(),
            cyw43_commit: "commit".into(),
            cyw43_assets: Vec::new(),
        }
    }

    fn boot() -> String {
        concat!(
            "{\"schema\":\"conduit-pico-w-signal/boot@1\",",
            "\"firmware_build_id\":\"firmware-build\",",
            "\"source_document_id\":\"source\",",
            "\"checked_form_id\":\"checked\",",
            "\"expanded_form_id\":\"expanded\",",
            "\"plan_id\":\"plan\",",
            "\"fragment_id\":\"fragment\",",
            "\"host_id\":\"host\",",
            "\"boot_id\":\"boot\",",
            "\"evidence_id\":\"boot-evidence\"}\n"
        )
        .to_owned()
    }

    fn receipt(sequence: usize) -> String {
        format!(
            concat!(
                "{{\"schema\":\"conduit-pico-w-signal/receipt@1\",",
                "\"firmware_build_id\":\"firmware-build\",",
                "\"source_document_id\":\"source\",",
                "\"checked_form_id\":\"checked\",",
                "\"expanded_form_id\":\"expanded\",",
                "\"plan_id\":\"plan\",",
                "\"fragment_id\":\"fragment\",",
                "\"host_id\":\"host\",",
                "\"boot_id\":\"boot\",",
                "\"active_play_id\":\"play\",",
                "\"sequence\":{},",
                "\"level\":{},",
                "\"presentation_id\":\"presentation-{}\",",
                "\"evidence_id\":\"evidence-{}\"}}\n"
            ),
            sequence,
            sequence % 2 == 1,
            sequence,
            sequence,
        )
    }

    fn terminal() -> String {
        concat!(
            "{\"schema\":\"conduit-pico-w-signal/terminal@1\",",
            "\"firmware_build_id\":\"firmware-build\",",
            "\"source_document_id\":\"source\",",
            "\"checked_form_id\":\"checked\",",
            "\"expanded_form_id\":\"expanded\",",
            "\"plan_id\":\"plan\",",
            "\"fragment_id\":\"fragment\",",
            "\"host_id\":\"host\",",
            "\"boot_id\":\"boot\",",
            "\"active_play_id\":\"play\",",
            "\"success\":true,",
            "\"evidence_id\":\"terminal-evidence\"}\n"
        )
        .to_owned()
    }

    #[test]
    fn accepts_exact_sixteen_receipts_and_terminal() {
        let mut input = String::new();
        input.push_str(&boot());
        for sequence in 0..EXPECTED_RECEIPTS {
            input.push_str(&receipt(sequence));
        }
        input.push_str(&terminal());
        verify_receipts(Cursor::new(input), &expected_firmware_identity())
            .expect("valid receipt stream");
    }

    #[test]
    fn rejects_reordered_receipt() {
        let input = format!("{}{}{}", boot(), receipt(1), terminal());
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }

    #[test]
    fn rejects_mutated_identity_field() {
        let mut input = String::new();
        input.push_str(&boot());
        for sequence in 0..EXPECTED_RECEIPTS {
            input.push_str(&receipt(sequence));
        }
        input.push_str(&terminal().replace("\"plan_id\":\"plan\"", "\"plan_id\":\"mutated\""));
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }

    #[test]
    fn rejects_mutated_firmware_build_identity() {
        let mut input = String::new();
        input.push_str(&boot());
        for sequence in 0..EXPECTED_RECEIPTS {
            input.push_str(&receipt(sequence));
        }
        input.push_str(&terminal().replace(
            "\"firmware_build_id\":\"firmware-build\"",
            "\"firmware_build_id\":\"other-build\"",
        ));
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }
}
