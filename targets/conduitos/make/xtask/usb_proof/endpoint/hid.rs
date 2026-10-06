//! Exact Source identity and independent HID transcript acceptance.
mod capture;
mod mouse;
use super::*;
use conduit_core::*;
use conduitos::{
    protocol_source::{PreparedProtocolSource, usb_hid_endpoint_package},
    usb_base::hid_endpoint_proof_plan,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HidSign {
    schema: String,
    proof_class: String,
    source_document_id: String,
    checked_plot_id: String,
    plan_id: String,
    active_play_id: String,
    device_instance_id: String,
    transfers: u16,
    cycle_transitions: u16,
    transcript_digest: String,
    normal_close: bool,
    acknowledged_stop: bool,
    fixture_protocol: bool,
    allocation_sealed: bool,
    capture_buffers: u8,
    maximum_pending_transfers: u8,
}

pub(super) fn retain(
    paths: &Paths,
    serial: &str,
    boot: &GuestBootSign,
    xhci: &GuestXhciSign,
    usb: &GuestUsbSign,
    mode: ProofMode,
) -> Result<(), ConduitosError> {
    let sign: HidSign = extract(serial, "CONDUIT_USB_HID_ENDPOINT_SIGN ")?;
    let artifact = hid_endpoint_proof_plan::plan(
        &EndpointReadProofSubject {
            host_id: &boot.host_id,
            boot_id: &boot.boot_id,
            controller_base_id: &usb.controller_base_id,
            device_instance_id: &usb.device_instance_id,
            root_port: usb.root_port,
            slot: usb.slot,
            attachment_epoch: usb.attachment_epoch,
            endpoint_dci: 3,
            endpoint_epoch: 1,
        },
        mode.entry()
            .ok_or_else(|| refusal("hid-proof-mode", "raw"))?,
    )
    .map_err(|error| refusal("hid-proof-plan", error))?;
    let plan = &artifact.artifact().definition().internal_plan;
    let expected = expected_digest(
        &artifact.artifact().definition().external_capability.outputs,
        mode,
    )?;
    validate_sign(plan, &usb.device_instance_id, &expected, &sign)?;
    let receipt = serde_json::json!({"schema":"conduit.conduitos.usb-hid-endpoint-proof/v1", "base_commit":git_head(&paths.root)?, "proof_class":"freestanding-emulator", "boot":boot, "controller":xhci, "device":usb, "hid":sign, "ordinary_class_offer":false, "five_architecture_acceptance":false});
    let output = paths.target.join(mode.receipt_name());
    fs::write(
        &output,
        serde_json::to_vec_pretty(&receipt)
            .map_err(|error| refusal("hid-proof-receipt", error.to_string()))?,
    )
    .map_err(|error| refusal("hid-proof-receipt", error.to_string()))?;
    println!("Source HID endpoint proof retained at {}", output.display());
    Ok(())
}

fn validate_sign(
    plan: &Plan,
    device: &str,
    expected: &str,
    sign: &HidSign,
) -> Result<(), ConduitosError> {
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let captures = fragment
        .placements
        .iter()
        .filter(|gear| {
            gear.implementation_id.as_str()
                == conduitos::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION
        })
        .count() as u8;
    for (field, wanted, actual) in [
        (
            "schema",
            "conduit.conduitos.usb-hid-endpoint/v1",
            sign.schema.as_str(),
        ),
        (
            "proof_class",
            "freestanding-emulator",
            sign.proof_class.as_str(),
        ),
        (
            "source_document_id",
            plan.source_document_id.as_str(),
            sign.source_document_id.as_str(),
        ),
        (
            "checked_plot_id",
            plan.checked_plot_id.as_str(),
            sign.checked_plot_id.as_str(),
        ),
        ("plan_id", plan.plan_id.as_str(), sign.plan_id.as_str()),
        (
            "active_play_id",
            active.active_play_id.as_str(),
            sign.active_play_id.as_str(),
        ),
        (
            "device_instance_id",
            device,
            sign.device_instance_id.as_str(),
        ),
        (
            "transcript_digest",
            expected,
            sign.transcript_digest.as_str(),
        ),
    ] {
        if wanted != actual {
            return Err(refusal(
                "hid-proof-sign",
                format!("{field}: expected {wanted}, actual {actual}"),
            ));
        }
    }
    for (field, wanted, actual) in [
        (
            "capture_buffers",
            u64::from(captures),
            u64::from(sign.capture_buffers),
        ),
        (
            "maximum_pending_transfers",
            u64::from(captures),
            u64::from(sign.maximum_pending_transfers),
        ),
        ("transfers", 128, u64::from(sign.transfers)),
        ("cycle_transitions", 2, u64::from(sign.cycle_transitions)),
    ] {
        if wanted != actual {
            return Err(refusal(
                "hid-proof-sign",
                format!("{field}: expected {wanted}, actual {actual}"),
            ));
        }
    }
    for (field, actual) in [
        ("normal_close", sign.normal_close),
        ("acknowledged_stop", sign.acknowledged_stop),
        ("fixture_protocol", sign.fixture_protocol),
        ("allocation_sealed", sign.allocation_sealed),
    ] {
        if !actual {
            return Err(refusal(
                "hid-proof-sign",
                format!("{field}: expected true, actual false"),
            ));
        }
    }
    Ok(())
}

fn expected_digest(outputs: &[PortDescriptor], mode: ProofMode) -> Result<String, ConduitosError> {
    if mode == ProofMode::Keyboard {
        return capture::expected_digest(outputs);
    }
    let source = PreparedProtocolSource::prepare(
        usb_hid_endpoint_package()
            .map_err(|error| refusal("hid-proof-source", format!("{error:?}")))?,
    )
    .map_err(|error| refusal("hid-proof-source", format!("{error:?}")))?;
    mouse::expected_digest(&source, outputs)
}
fn field_type(ty: &StructuredInfoType, name: &str) -> Result<StructuredInfoType, ConduitosError> {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err(refusal("hid-proof-type", "record"));
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value_type().clone())
        .ok_or_else(|| refusal("hid-proof-field", name))
}
fn leaf(
    ty: &StructuredInfoType,
    name: &str,
    bytes: Vec<u8>,
) -> Result<StructuredFieldValue, ConduitosError> {
    StructuredFieldValue::new(
        name,
        StructuredInfoValue::leaf(field_type(ty, name)?, bytes).map_err(value_error)?,
    )
    .map_err(value_error)
}
fn record(
    ty: &StructuredInfoType,
    fields: Vec<StructuredFieldValue>,
) -> Result<StructuredInfoValue, ConduitosError> {
    StructuredInfoValue::record(ty.clone(), fields).map_err(value_error)
}
fn value_error(error: StructuredInfoRefusal) -> ConduitosError {
    refusal("hid-proof-value", format!("{error:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_rejects_substituted_identity_transcript_and_false_lifecycle() {
        for mode in [ProofMode::Keyboard, ProofMode::Mouse] {
            let artifact = hid_endpoint_proof_plan::plan(
                &EndpointReadProofSubject {
                    host_id: "proof/host",
                    boot_id: "proof/boot",
                    controller_base_id: "proof/controller",
                    device_instance_id: "proof/device",
                    root_port: 1,
                    slot: 1,
                    attachment_epoch: 1,
                    endpoint_dci: 3,
                    endpoint_epoch: 1,
                },
                mode.entry().unwrap(),
            )
            .unwrap();
            let definition = artifact.artifact().definition();
            let plan = &definition.internal_plan;
            let fragment = &plan.fragments[0];
            let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
            let expected = expected_digest(&definition.external_capability.outputs, mode).unwrap();
            let specimen = HidSign {
                schema: "conduit.conduitos.usb-hid-endpoint/v1".into(),
                proof_class: "freestanding-emulator".into(),
                source_document_id: plan.source_document_id.as_str().into(),
                checked_plot_id: plan.checked_plot_id.as_str().into(),
                plan_id: plan.plan_id.as_str().into(),
                active_play_id: active.active_play_id.as_str().into(),
                device_instance_id: "proof/device".into(),
                transfers: 128,
                cycle_transitions: 2,
                transcript_digest: expected.clone(),
                normal_close: true,
                acknowledged_stop: true,
                fixture_protocol: true,
                allocation_sealed: true,
                capture_buffers: if mode == ProofMode::Keyboard { 8 } else { 1 },
                maximum_pending_transfers: if mode == ProofMode::Keyboard { 8 } else { 1 },
            };
            validate_sign(plan, "proof/device", &expected, &specimen).unwrap();
            for (field, replacement) in [
                ("schema", serde_json::json!("wrong")),
                ("proof_class", serde_json::json!("simulation")),
                ("source_document_id", serde_json::json!("wrong")),
                ("checked_plot_id", serde_json::json!("wrong")),
                ("plan_id", serde_json::json!("wrong")),
                ("active_play_id", serde_json::json!("wrong")),
                ("device_instance_id", serde_json::json!("wrong")),
                ("transfers", serde_json::json!(127)),
                ("cycle_transitions", serde_json::json!(1)),
                ("transcript_digest", serde_json::json!("wrong")),
                ("normal_close", serde_json::json!(false)),
                ("acknowledged_stop", serde_json::json!(false)),
                ("fixture_protocol", serde_json::json!(false)),
                ("allocation_sealed", serde_json::json!(false)),
                ("capture_buffers", serde_json::json!(0)),
                ("maximum_pending_transfers", serde_json::json!(0)),
            ] {
                let mut forged = serde_json::to_value(&specimen).unwrap();
                forged[field] = replacement;
                let forged: HidSign = serde_json::from_value(forged).unwrap();
                assert!(
                    validate_sign(plan, "proof/device", &expected, &forged).is_err(),
                    "{field}"
                );
            }
        }
    }
}
