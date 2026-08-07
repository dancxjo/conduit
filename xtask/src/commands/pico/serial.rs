use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::doctor::repo_root;
use super::firmware::{read_identity_manifest, FirmwareIdentity, GeneratedImageIdentity};
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

    let start = Instant::now();
    let (_, port) = loop {
        match resolve_dual_ports(None, args.port.as_deref()) {
            Ok(ports) => break ports,
            Err(_) if start.elapsed() < Duration::from_secs(10) => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(error),
        }
    };
    println!("==> pico verify: reading receipts from {}", port.display());

    let identity = read_identity_manifest(&repo_root())?;
    if identity.firmware_mode != "pico-local" {
        return Err(format!(
            "pico verify requires a pico-local image, but the current artifact is {}; rebuild with `cargo xtask pico build`",
            identity.firmware_mode
        )
        .into());
    }
    let file = std::fs::OpenOptions::new().read(true).open(&port)?;
    conduit_std_host::usb_cdc::configure_cdc_port(&file, 0, 50).map_err(|e| {
        format!(
            "Failed to configure evidence serial port {}: {}",
            port.display(),
            e
        )
    })?;
    verify_receipts(BufReader::new(file), &identity)
}

fn verify_receipts(reader: impl BufRead, identity: &FirmwareIdentity) -> PicoResult<()> {
    validate_expected_identity(identity)?;
    let mut boot_seen = false;
    let mut receipts = 0usize;
    let mut terminal_seen = false;
    let mut runtime_identity: Option<RuntimeTranscriptIdentity> = None;

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
            let runtime = verify_boot_identity(&record, identity)?;
            runtime_identity = Some(runtime);
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
            verify_terminal_identity(
                &record,
                identity,
                runtime_identity
                    .as_ref()
                    .ok_or("terminal record arrived before runtime boot identity")?,
            )?;
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
            verify_presentation_identity(
                &record,
                receipts,
                identity,
                runtime_identity
                    .as_ref()
                    .ok_or("presentation receipt arrived before runtime boot identity")?,
            )?;
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
    if identity.firmware_mode != expected.firmware_mode {
        return Err(format!(
            "identity manifest firmware mode mismatch: top-level {}, generated image {}",
            identity.firmware_mode, expected.firmware_mode
        )
        .into());
    }
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

fn verify_boot_identity(
    record: &serde_json::Value,
    identity: &FirmwareIdentity,
) -> PicoResult<RuntimeTranscriptIdentity> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, false)?;
    verify_field(record, "evidence_id", &expected.boot_evidence_id)?;
    read_runtime_identity(record, expected)
}

fn verify_presentation_identity(
    record: &serde_json::Value,
    sequence: usize,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, true)?;
    verify_runtime_identity(record, runtime)?;
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
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let expected = &identity.generated_image;
    verify_static_identity(record, identity, true)?;
    verify_runtime_identity(record, runtime)?;
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct RuntimeTranscriptIdentity {
    boot_id: String,
    active_play_id: String,
}

fn read_runtime_identity(
    record: &serde_json::Value,
    expected: &GeneratedImageIdentity,
) -> PicoResult<RuntimeTranscriptIdentity> {
    let boot_id = read_runtime_field(record, "runtime_boot_id")?;
    let active_play_id = read_runtime_field(record, "runtime_active_play_id")?;
    if boot_id == expected.boot_id {
        return Err("runtime_boot_id must be distinct from planned boot_id".into());
    }
    if active_play_id == expected.active_play_id {
        return Err("runtime_active_play_id must be distinct from planned active_play_id".into());
    }
    let canonical = conduit_core::bind_active_play(
        &conduit_core::PlanId::from(expected.plan_id.as_str()),
        &conduit_core::HostId::from(expected.host_id.as_str()),
        &conduit_core::BootId::from(boot_id.as_str()),
        0,
    )
    .active_play_id;
    if active_play_id != canonical.as_str() {
        return Err("runtime_active_play_id is not canonically bound to plan/host/boot".into());
    }
    Ok(RuntimeTranscriptIdentity {
        boot_id,
        active_play_id,
    })
}

fn verify_runtime_identity(
    record: &serde_json::Value,
    expected: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let actual = RuntimeTranscriptIdentity {
        boot_id: read_runtime_field(record, "runtime_boot_id")?,
        active_play_id: read_runtime_field(record, "runtime_active_play_id")?,
    };
    if &actual != expected {
        return Err(format!(
            "runtime transcript identity changed: expected boot {} play {}, got boot {} play {}",
            expected.boot_id, expected.active_play_id, actual.boot_id, actual.active_play_id
        )
        .into());
    }
    Ok(())
}

fn read_runtime_field(record: &serde_json::Value, field: &str) -> PicoResult<String> {
    let value = record[field]
        .as_str()
        .ok_or_else(|| format!("receipt missing runtime identity field `{field}`"))?;
    if value.is_empty() {
        return Err(format!("runtime identity field `{field}` must not be empty").into());
    }
    Ok(value.to_owned())
}

pub fn resolve_dual_ports(
    link_arg: Option<&str>,
    evidence_arg: Option<&str>,
) -> PicoResult<(PathBuf, PathBuf)> {
    if let (Some(l), Some(e)) = (link_arg, evidence_arg) {
        return Ok((PathBuf::from(l), PathBuf::from(e)));
    }

    let by_id = PathBuf::from("/dev/serial/by-id");
    if by_id.is_dir() {
        let mut matches = std::fs::read_dir(&by_id)?
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

        matches.sort();

        if matches.len() >= 2 {
            let link = link_arg
                .map(PathBuf::from)
                .unwrap_or_else(|| matches[0].clone());
            let evidence = evidence_arg
                .map(PathBuf::from)
                .unwrap_or_else(|| matches[1].clone());
            return Ok((link, evidence));
        } else if matches.len() == 1 {
            let single = matches[0].clone();
            return Ok((
                link_arg
                    .map(PathBuf::from)
                    .unwrap_or_else(|| single.clone()),
                evidence_arg.map(PathBuf::from).unwrap_or(single),
            ));
        }
    }

    if let (Some(l), None) = (link_arg, evidence_arg) {
        return Ok((PathBuf::from(l), PathBuf::from(l)));
    }

    Err("no Conduit Pico W serial device found under /dev/serial/by-id; pass --link-port and --evidence-port".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn expected_identity() -> GeneratedImageIdentity {
        GeneratedImageIdentity {
            schema: "conduit.pico-signal.generated-image@1".into(),
            firmware_mode: "pico-local".into(),
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
            firmware_mode: "pico-local".into(),
            firmware_build_id: "firmware-build".into(),
            firmware_sha256: "sha256".into(),
            generated_image: expected_identity(),
            cyw43_commit: "commit".into(),
            cyw43_assets: Vec::new(),
        }
    }

    fn boot() -> String {
        format!(
            concat!(
                "{{\"schema\":\"conduit-pico-w-signal/boot@1\",",
                "\"firmware_build_id\":\"firmware-build\",",
                "\"source_document_id\":\"source\",",
                "\"checked_form_id\":\"checked\",",
                "\"expanded_form_id\":\"expanded\",",
                "\"plan_id\":\"plan\",",
                "\"fragment_id\":\"fragment\",",
                "\"host_id\":\"host\",",
                "\"boot_id\":\"boot\",",
                "\"runtime_boot_id\":\"runtime-boot\",",
                "\"runtime_active_play_id\":\"{}\",",
                "\"evidence_id\":\"boot-evidence\"}}\n"
            ),
            runtime_play(),
        )
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
                "\"runtime_boot_id\":\"runtime-boot\",",
                "\"runtime_active_play_id\":\"{}\",",
                "\"sequence\":{},",
                "\"level\":{},",
                "\"presentation_id\":\"presentation-{}\",",
                "\"evidence_id\":\"evidence-{}\"}}\n"
            ),
            runtime_play(),
            sequence,
            sequence % 2 == 1,
            sequence,
            sequence,
        )
    }

    fn terminal() -> String {
        format!(
            concat!(
                "{{\"schema\":\"conduit-pico-w-signal/terminal@1\",",
                "\"firmware_build_id\":\"firmware-build\",",
                "\"source_document_id\":\"source\",",
                "\"checked_form_id\":\"checked\",",
                "\"expanded_form_id\":\"expanded\",",
                "\"plan_id\":\"plan\",",
                "\"fragment_id\":\"fragment\",",
                "\"host_id\":\"host\",",
                "\"boot_id\":\"boot\",",
                "\"active_play_id\":\"play\",",
                "\"runtime_boot_id\":\"runtime-boot\",",
                "\"runtime_active_play_id\":\"{}\",",
                "\"success\":true,",
                "\"evidence_id\":\"terminal-evidence\"}}\n"
            ),
            runtime_play(),
        )
    }

    fn runtime_play() -> String {
        conduit_core::bind_active_play(
            &conduit_core::PlanId::from("plan"),
            &conduit_core::HostId::from("host"),
            &conduit_core::BootId::from("runtime-boot"),
            0,
        )
        .active_play_id
        .as_str()
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

    #[test]
    fn rejects_missing_runtime_identity_field() {
        let input = format!(
            "{}{}{}",
            boot().replace("\"runtime_boot_id\":\"runtime-boot\",", ""),
            (0..EXPECTED_RECEIPTS).map(receipt).collect::<String>(),
            terminal()
        );
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }

    #[test]
    fn rejects_runtime_identity_reusing_planned_identity() {
        let input = format!(
            "{}{}{}",
            boot()
                .replace(
                    "\"runtime_boot_id\":\"runtime-boot\"",
                    "\"runtime_boot_id\":\"boot\"",
                )
                .replace(
                    &format!("\"runtime_active_play_id\":\"{}\"", runtime_play()),
                    "\"runtime_active_play_id\":\"play\""
                ),
            (0..EXPECTED_RECEIPTS).map(receipt).collect::<String>(),
            terminal()
        );
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }

    #[test]
    fn rejects_runtime_identity_change_after_boot() {
        let mut input = String::new();
        input.push_str(&boot());
        for sequence in 0..EXPECTED_RECEIPTS {
            input.push_str(&receipt(sequence));
        }
        input.push_str(&terminal().replace(
            &format!("\"runtime_active_play_id\":\"{}\"", runtime_play()),
            "\"runtime_active_play_id\":\"other-runtime-play\"",
        ));
        assert!(verify_receipts(Cursor::new(input), &expected_firmware_identity()).is_err());
    }
}
