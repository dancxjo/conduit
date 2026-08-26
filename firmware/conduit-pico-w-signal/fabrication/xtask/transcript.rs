use super::firmware::FirmwareIdentity;
use super::PicoResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeTranscriptIdentity {
    pub boot_id: String,
    pub active_play_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct BluetoothTranscriptSummary {
    pub runtime_boot_id: String,
    pub runtime_active_play_id: String,
    pub paired_marker_seen: bool,
}

pub fn verify_bluetooth_transcript(
    lines: &[String],
    identity: &FirmwareIdentity,
) -> PicoResult<BluetoothTranscriptSummary> {
    if identity.firmware_mode != "bluetooth-line" {
        return Err(format!(
            "Bluetooth transcript requires a bluetooth-line image, found {}",
            identity.firmware_mode
        )
        .into());
    }
    let mut boot = None;
    let mut receipts = 0_usize;
    let mut terminal_seen = false;
    let mut controller_seen = false;
    let mut connected_seen = false;
    let mut complete_seen = false;
    let mut paired_marker_seen = false;
    for line in lines {
        match line.as_str() {
            "CONDUIT_BLE_CONTROLLER_READY" => controller_seen = true,
            "CONDUIT_BLE_LINE_CONNECTED" => connected_seen = true,
            "CONDUIT_BLE_PEER_PAIRED" => paired_marker_seen = true,
            "CONDUIT_BLE_LINE_COMPLETE" => complete_seen = true,
            "CONDUIT_BLE_LINE_LOST" => {
                return Err("Pico reported BLE Line loss during successful proof".into())
            }
            _ if line.contains("\"schema\":\"conduit-pico-w-signal/boot@1\"") => {
                if boot.is_some() {
                    return Err("duplicate Bluetooth runtime boot receipt".into());
                }
                boot = Some(verify_boot(line, identity)?);
            }
            _ if line.contains("\"schema\":\"conduit-pico-w-signal/receipt@1\"") => {
                verify_receipt(
                    line,
                    receipts,
                    conduit_signal::signal_level_for_sequence(receipts as u64, true),
                    identity,
                    boot.as_ref()
                        .ok_or("Bluetooth receipt arrived before boot")?,
                )?;
                receipts += 1;
            }
            _ if line.contains("\"schema\":\"conduit-pico-w-signal/terminal@1\"") => {
                verify_terminal(
                    line,
                    identity,
                    boot.as_ref()
                        .ok_or("Bluetooth terminal arrived before boot")?,
                )?;
                terminal_seen = true;
            }
            _ => {}
        }
    }
    let expected_receipts = identity.generated_image.presentation_ids.len();
    if !controller_seen
        || !connected_seen
        || receipts != expected_receipts
        || !terminal_seen
        || !complete_seen
    {
        return Err(format!(
            "incomplete Bluetooth transcript: controller={controller_seen}, connected={connected_seen}, receipts={receipts}/{expected_receipts}, terminal={terminal_seen}, complete={complete_seen}"
        )
        .into());
    }
    let runtime = boot.ok_or("Bluetooth transcript has no runtime boot receipt")?;
    Ok(BluetoothTranscriptSummary {
        runtime_boot_id: runtime.boot_id,
        runtime_active_play_id: runtime.active_play_id,
        paired_marker_seen,
    })
}

pub fn verify_bluetooth_loss_transcript(
    lines: &[String],
    identity: &FirmwareIdentity,
) -> PicoResult<BluetoothTranscriptSummary> {
    if identity.firmware_mode != "bluetooth-line" {
        return Err(format!(
            "Bluetooth loss transcript requires a bluetooth-line image, found {}",
            identity.firmware_mode
        )
        .into());
    }
    let mut boot = None;
    let mut controller_seen = false;
    let mut connected_seen = false;
    let mut loss_seen = false;
    let mut paired_marker_seen = false;
    for line in lines {
        match line.as_str() {
            "CONDUIT_BLE_CONTROLLER_READY" => controller_seen = true,
            "CONDUIT_BLE_LINE_CONNECTED" => connected_seen = true,
            "CONDUIT_BLE_PEER_PAIRED" => paired_marker_seen = true,
            "CONDUIT_BLE_LINE_LOST" => loss_seen = true,
            "CONDUIT_BLE_LINE_COMPLETE" => {
                return Err(
                    "Pico reported successful completion during transport-loss proof".into(),
                )
            }
            _ if line.contains("\"schema\":\"conduit-pico-w-signal/boot@1\"") => {
                if boot.is_some() {
                    return Err("duplicate Bluetooth runtime boot receipt".into());
                }
                boot = Some(verify_boot(line, identity)?);
            }
            _ if line.contains("\"schema\":\"conduit-pico-w-signal/receipt@1\"")
                || line.contains("\"schema\":\"conduit-pico-w-signal/terminal@1\"") =>
            {
                return Err("Bluetooth loss proof observed success receipt state".into())
            }
            _ => {}
        }
    }
    if !controller_seen || !connected_seen || !loss_seen {
        return Err(format!(
            "incomplete Bluetooth loss transcript: controller={controller_seen}, connected={connected_seen}, loss={loss_seen}"
        )
        .into());
    }
    let runtime = boot.ok_or("Bluetooth loss transcript has no runtime boot receipt")?;
    Ok(BluetoothTranscriptSummary {
        runtime_boot_id: runtime.boot_id,
        runtime_active_play_id: runtime.active_play_id,
        paired_marker_seen,
    })
}

pub fn verify_boot(
    line: &str,
    identity: &FirmwareIdentity,
) -> PicoResult<RuntimeTranscriptIdentity> {
    let record = parse_schema(line, "conduit-pico-w-signal/boot@1")?;
    verify_static(&record, identity, false)?;
    verify_field(&record, "sign_id", &identity.generated_image.boot_sign_id)?;
    let runtime = RuntimeTranscriptIdentity {
        boot_id: required_string(&record, "runtime_boot_id")?.to_owned(),
        active_play_id: required_string(&record, "runtime_active_play_id")?.to_owned(),
    };
    if runtime.boot_id == identity.generated_image.boot_id
        || runtime.active_play_id == identity.generated_image.active_play_id
    {
        return Err("runtime transcript identity reused generated-image identity".into());
    }
    let expected_play = conduit_core::bind_active_play(
        &conduit_core::PlanId::from(identity.generated_image.plan_id.as_str()),
        &conduit_core::HostId::from(identity.generated_image.host_id.as_str()),
        &conduit_core::BootId::from(runtime.boot_id.as_str()),
        0,
    )
    .active_play_id;
    if runtime.active_play_id != expected_play.as_str() {
        return Err(
            "runtime active-play identity is not canonically bound to plan/host/boot".into(),
        );
    }
    Ok(runtime)
}

pub fn verify_receipt(
    line: &str,
    sequence: usize,
    level: bool,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let record = parse_schema(line, "conduit-pico-w-signal/receipt@1")?;
    verify_static(&record, identity, true)?;
    verify_runtime(&record, runtime)?;
    if record["sequence"].as_u64() != Some(sequence as u64) {
        return Err(format!("receipt sequence mismatch for item {sequence}: {line}").into());
    }
    if record["level"].as_bool() != Some(level) {
        return Err(format!("receipt level mismatch for item {sequence}: {line}").into());
    }
    verify_field(
        &record,
        "presentation_id",
        identity
            .generated_image
            .presentation_ids
            .get(sequence)
            .ok_or("identity manifest has no presentation ID for receipt")?,
    )?;
    verify_field(
        &record,
        "sign_id",
        identity
            .generated_image
            .presentation_sign_ids
            .get(sequence)
            .ok_or("identity manifest has no sign ID for receipt")?,
    )
}

pub fn verify_terminal(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    verify_terminal_disposition(line, identity, runtime, true)
}

pub fn verify_terminal_failure(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    verify_terminal_disposition(line, identity, runtime, false)
}

fn verify_terminal_disposition(
    line: &str,
    identity: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
    expected_success: bool,
) -> PicoResult<()> {
    let record = parse_schema(line, "conduit-pico-w-signal/terminal@1")?;
    verify_static(&record, identity, true)?;
    verify_runtime(&record, runtime)?;
    if record["success"].as_bool() != Some(expected_success) {
        return Err(format!("Pico terminal disposition disagrees with proof: {line}").into());
    }
    verify_field(
        &record,
        "sign_id",
        &identity.generated_image.terminal_sign_id,
    )
}

fn parse_schema(line: &str, schema: &str) -> PicoResult<serde_json::Value> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed Pico transcript JSON: {error}; line: {line}"))?;
    verify_field(&record, "schema", schema)?;
    Ok(record)
}

fn verify_static(
    record: &serde_json::Value,
    identity: &FirmwareIdentity,
    active_play: bool,
) -> PicoResult<()> {
    let generated = &identity.generated_image;
    for (field, expected) in [
        ("firmware_build_id", identity.firmware_build_id.as_str()),
        ("source_document_id", generated.source_document_id.as_str()),
        ("checked_form_id", generated.checked_form_id.as_str()),
        ("expanded_form_id", generated.expanded_form_id.as_str()),
        ("plan_id", generated.plan_id.as_str()),
        ("fragment_id", generated.fragment_id.as_str()),
        ("host_id", generated.host_id.as_str()),
        ("boot_id", generated.boot_id.as_str()),
    ] {
        verify_field(record, field, expected)?;
    }
    if active_play {
        verify_field(record, "active_play_id", &generated.active_play_id)?;
    }
    Ok(())
}

fn verify_runtime(
    record: &serde_json::Value,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    verify_field(record, "runtime_boot_id", &runtime.boot_id)?;
    verify_field(record, "runtime_active_play_id", &runtime.active_play_id)
}

fn verify_field(record: &serde_json::Value, field: &str, expected: &str) -> PicoResult<()> {
    let actual = required_string(record, field)?;
    if actual != expected {
        return Err(format!(
            "Pico transcript field `{field}` mismatch: expected {expected}, got {actual}"
        )
        .into());
    }
    Ok(())
}

fn required_string<'a>(record: &'a serde_json::Value, field: &str) -> PicoResult<&'a str> {
    let value = record[field]
        .as_str()
        .ok_or_else(|| format!("Pico transcript missing string field `{field}`"))?;
    if value.is_empty() {
        return Err(format!("Pico transcript field `{field}` is empty").into());
    }
    Ok(value)
}
