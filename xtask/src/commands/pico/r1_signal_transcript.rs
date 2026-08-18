//! Exact verifier for the generated R1 Pico Signal execution Signs.

use conduit_core::BootId;

use super::firmware::FirmwareIdentity;
use super::transcript::RuntimeTranscriptIdentity;
use super::PicoResult;

pub fn verify_receipt(
    line: &str,
    plan: &conduit_core::Plan,
    sequence: u64,
    expected_level: bool,
    firmware: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed physical LED Sign: {error}"))?;
    verify_identity(&record, plan, firmware, runtime)?;
    let fragment = pico_fragment(plan)?;
    let placement = fragment
        .placements
        .iter()
        .find(|placement| placement.gear_id.as_str() == "signal-demo/show")
        .ok_or("R1 Pico fragment has no Show placement")?;
    let planned_play =
        conduit_core::bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let presentation = conduit_core::bind_presentation(
        &planned_play.active_play_id,
        &placement.placement_id,
        sequence,
    );
    let sign = conduit_core::bind_sign(
        &fragment.host_id,
        &fragment.boot_id,
        Some(&planned_play.active_play_id),
        sequence,
    );
    for (field, expected) in [
        ("schema", "conduit-pico-w-signal/receipt@1"),
        ("presentation_id", presentation.presentation_id.as_str()),
        ("sign_id", sign.sign_id.as_str()),
    ] {
        if record[field].as_str() != Some(expected) {
            return Err(format!("physical LED Sign field `{field}` mismatched").into());
        }
    }
    if record["sequence"].as_u64() != Some(sequence)
        || record["level"].as_bool() != Some(expected_level)
    {
        return Err("physical LED Sign sequence or level mismatched".into());
    }
    Ok(())
}

pub fn verify_terminal(
    line: &str,
    plan: &conduit_core::Plan,
    firmware: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let record: serde_json::Value = serde_json::from_str(line)
        .map_err(|error| format!("malformed Plan B terminal Sign: {error}"))?;
    verify_identity(&record, plan, firmware, runtime)?;
    let fragment = pico_fragment(plan)?;
    let planned_play =
        conduit_core::bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let sign = conduit_core::bind_sign(
        &fragment.host_id,
        &fragment.boot_id,
        Some(&planned_play.active_play_id),
        16,
    );
    if record["schema"].as_str() != Some("conduit-pico-w-signal/terminal@1")
        || record["success"].as_bool() != Some(true)
        || record["sign_id"].as_str() != Some(sign.sign_id.as_str())
    {
        return Err("Plan B terminal Sign identity or disposition mismatched".into());
    }
    Ok(())
}

fn verify_identity(
    record: &serde_json::Value,
    plan: &conduit_core::Plan,
    firmware: &FirmwareIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> PicoResult<()> {
    let fragment = pico_fragment(plan)?;
    let planned_play =
        conduit_core::bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let runtime_play = conduit_core::bind_active_play(
        &plan.plan_id,
        &fragment.host_id,
        &BootId::from(runtime.boot_id.as_str()),
        0,
    );
    let expected_firmware_build = if firmware.firmware_mode == "r1-control" {
        firmware
            .verified_r1_control_image(&plan.plan_id)?
            .firmware_build_id
            .clone()
    } else {
        let suffix = format!(
            ":{}:{}",
            firmware.generated_image.plan_id, firmware.generated_image.fragment_id
        );
        let prefix = firmware
            .firmware_build_id
            .strip_suffix(&suffix)
            .ok_or("network firmware build identity has an unexpected suffix")?;
        format!(
            "{prefix}:{}:{}",
            plan.plan_id.as_str(),
            fragment.fragment_id.as_str()
        )
    };
    for (field, expected) in [
        ("firmware_build_id", expected_firmware_build.as_str()),
        ("source_document_id", plan.source_document_id.as_str()),
        ("checked_form_id", plan.checked_form_id.as_str()),
        ("expanded_form_id", plan.expanded_form_id.as_str()),
        ("plan_id", plan.plan_id.as_str()),
        ("fragment_id", fragment.fragment_id.as_str()),
        ("host_id", fragment.host_id.as_str()),
        ("boot_id", fragment.boot_id.as_str()),
        ("active_play_id", planned_play.active_play_id.as_str()),
        ("runtime_boot_id", runtime.boot_id.as_str()),
        (
            "runtime_active_play_id",
            runtime_play.active_play_id.as_str(),
        ),
    ] {
        if record[field].as_str() != Some(expected) {
            return Err(format!("Signal execution Sign field `{field}` mismatched").into());
        }
    }
    Ok(())
}

fn pico_fragment(plan: &conduit_core::Plan) -> PicoResult<&conduit_core::PlanFragment> {
    plan.fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
        .ok_or_else(|| "R1 Plan has no Pico fragment".into())
}

#[cfg(test)]
mod tests {
    use conduit_core::{bind_active_play, bind_presentation, bind_sign, BootId};

    use super::super::firmware::{FirmwareIdentity, GeneratedImageIdentity};
    use super::*;

    fn fixture() -> (
        conduit_core::Plan,
        FirmwareIdentity,
        RuntimeTranscriptIdentity,
    ) {
        let plan = conduit_system_continuity::exact_r1_signal_plan(
            BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap()
        .plan;
        let network = conduit_net::exact_r1_network_bootstrap_plan().unwrap().plan;
        let network_fragment = network
            .fragments
            .iter()
            .find(|fragment| fragment.host_id.as_str() == conduit_net::R1_PICO_HOST_ID)
            .unwrap();
        let prefix = "conduit-pico-w-signal:test:clean:thumb:release:wifi-bootstrap";
        let firmware_build_id = format!(
            "{prefix}:{}:{}",
            network.plan_id.as_str(),
            network_fragment.fragment_id.as_str()
        );
        let firmware = FirmwareIdentity {
            schema: "conduit-pico-w-signal/identity@1".into(),
            git_revision: "test".into(),
            target: "thumbv6m-none-eabi".into(),
            profile: "release".into(),
            firmware_mode: "wifi-bootstrap".into(),
            firmware_build_id: firmware_build_id.clone(),
            firmware_sha256: "test".into(),
            generated_image: GeneratedImageIdentity {
                schema: "conduit.pico-network.generated-image@1".into(),
                firmware_mode: "wifi-bootstrap".into(),
                firmware_build_id,
                source_document_id: network.source_document_id.as_str().into(),
                checked_form_id: network.checked_form_id.as_str().into(),
                expanded_form_id: network.expanded_form_id.as_str().into(),
                plan_id: network.plan_id.as_str().into(),
                fragment_id: network_fragment.fragment_id.as_str().into(),
                host_id: network_fragment.host_id.as_str().into(),
                boot_id: network_fragment.boot_id.as_str().into(),
                active_play_id: "network-play".into(),
                boot_sign_id: "network-boot-sign".into(),
                presentation_ids: vec![],
                presentation_sign_ids: vec![],
                terminal_sign_id: "network-terminal-sign".into(),
                offer_generation: 1,
                nodes: 2,
                cords: 2,
                host_operations: 2,
                cord_value_slots: 2,
                cord_value_bytes: 1,
                sign_items: 1,
                sign_bytes: 1,
            },
            r1_control_images: None,
            cyw43_commit: "test".into(),
            cyw43_assets: vec![],
        };
        let runtime = RuntimeTranscriptIdentity {
            boot_id: "r1/runtime-pico-boot".into(),
            active_play_id: "network-runtime-play".into(),
        };
        (plan, firmware, runtime)
    }

    #[test]
    fn receipt_verifier_correlates_every_plan_and_runtime_identity() {
        let (plan, firmware, runtime) = fixture();
        let fragment = pico_fragment(&plan).unwrap();
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.gear_id.as_str() == "signal-demo/show")
            .unwrap();
        let planned_play = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let runtime_play = bind_active_play(
            &plan.plan_id,
            &fragment.host_id,
            &BootId::from(runtime.boot_id.as_str()),
            0,
        );
        let presentation =
            bind_presentation(&planned_play.active_play_id, &placement.placement_id, 0);
        let sign = bind_sign(
            &fragment.host_id,
            &fragment.boot_id,
            Some(&planned_play.active_play_id),
            0,
        );
        let prefix = firmware
            .firmware_build_id
            .strip_suffix(&format!(
                ":{}:{}",
                firmware.generated_image.plan_id, firmware.generated_image.fragment_id
            ))
            .unwrap();
        let mut record = serde_json::json!({
            "schema": "conduit-pico-w-signal/receipt@1",
            "firmware_build_id": format!("{prefix}:{}:{}", plan.plan_id.as_str(), fragment.fragment_id.as_str()),
            "source_document_id": plan.source_document_id.as_str(),
            "checked_form_id": plan.checked_form_id.as_str(),
            "expanded_form_id": plan.expanded_form_id.as_str(),
            "plan_id": plan.plan_id.as_str(),
            "fragment_id": fragment.fragment_id.as_str(),
            "host_id": fragment.host_id.as_str(),
            "boot_id": fragment.boot_id.as_str(),
            "active_play_id": planned_play.active_play_id.as_str(),
            "runtime_boot_id": runtime.boot_id,
            "runtime_active_play_id": runtime_play.active_play_id.as_str(),
            "sequence": 0,
            "level": false,
            "presentation_id": presentation.presentation_id.as_str(),
            "sign_id": sign.sign_id.as_str(),
        });
        verify_receipt(&record.to_string(), &plan, 0, false, &firmware, &runtime).unwrap();
        record["plan_id"] = serde_json::Value::String("stale-plan".into());
        assert!(verify_receipt(&record.to_string(), &plan, 0, false, &firmware, &runtime).is_err());
    }
}
