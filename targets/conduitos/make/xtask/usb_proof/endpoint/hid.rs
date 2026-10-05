//! Exact Source identity and independent keyboard transcript acceptance.
use super::*;
use conduit_core::*;
use conduitos::{
    protocol_source::{usb_hid_endpoint_package, PreparedProtocolSource},
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
}

pub(super) fn retain(
    paths: &Paths,
    serial: &str,
    boot: &GuestBootSign,
    xhci: &GuestXhciSign,
    usb: &GuestUsbSign,
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
        "usb-hid-keyboard-endpoint",
    )
    .map_err(|error| refusal("hid-proof-plan", error))?;
    let plan = &artifact.artifact().definition().internal_plan;
    let expected = expected_digest(&artifact.artifact().definition().external_capability.outputs)?;
    validate_sign(plan, &usb.device_instance_id, &expected, &sign)?;
    let receipt = serde_json::json!({"schema":"conduit.conduitos.usb-hid-endpoint-proof/v1", "base_commit":git_head(&paths.root)?, "proof_class":"freestanding-emulator", "boot":boot, "controller":xhci, "device":usb, "hid":sign, "ordinary_class_offer":false, "five_architecture_acceptance":false});
    let output = paths.target.join("usb-hid-endpoint-proof.json");
    fs::write(
        &output,
        serde_json::to_vec_pretty(&receipt)
            .map_err(|error| refusal("hid-proof-receipt", error.to_string()))?,
    )
    .map_err(|error| refusal("hid-proof-receipt", error.to_string()))?;
    println!(
        "Source keyboard endpoint proof retained at {}",
        output.display()
    );
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
    if sign.schema != "conduit.conduitos.usb-hid-endpoint/v1"
        || sign.proof_class != "freestanding-emulator"
        || sign.source_document_id != plan.source_document_id.as_str()
        || sign.checked_plot_id != plan.checked_plot_id.as_str()
        || sign.plan_id != plan.plan_id.as_str()
        || sign.active_play_id != active.active_play_id.as_str()
        || sign.device_instance_id != device
        || sign.transfers != 128
        || sign.cycle_transitions != 2
        || sign.transcript_digest != expected
        || !sign.normal_close
        || !sign.acknowledged_stop
        || !sign.fixture_protocol
    {
        return Err(refusal("hid-proof-sign", format!("{sign:?}")));
    }
    Ok(())
}

fn expected_digest(outputs: &[PortDescriptor]) -> Result<String, ConduitosError> {
    let source = PreparedProtocolSource::prepare(
        usb_hid_endpoint_package()
            .map_err(|error| refusal("hid-proof-source", format!("{error:?}")))?,
    )
    .map_err(|error| refusal("hid-proof-source", format!("{error:?}")))?;
    let schema = |name: &str| {
        source
            .checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .map(|ty| ty.value_type.clone())
            .ok_or_else(|| refusal("hid-proof-type", name))
    };
    let frame = schema("UsbHidReportFrame")?;
    let transfer = schema("UsbHidTransferFrame")?;
    let report = schema("UsbBootKeyboardReport")?;
    let observed = schema("UsbBootKeyboardResult")?;
    let transition = schema("UsbKeyboardTransition")?;
    let mut digest = Sha256::new();
    for sequence in 0..128_u64 {
        let pressed = sequence % 2 == 0;
        let wire = vec![0, 0, if pressed { 4 } else { 0 }, 0, 0, 0, 0, 0];
        let received = StructuredInfoValue::variant(
            transfer.clone(),
            "frame",
            record(
                &frame,
                vec![
                    leaf(&frame, "wire", wire)?,
                    leaf(&frame, "actual", 8_u64.to_le_bytes().to_vec())?,
                ],
            )?,
        )
        .map_err(value_error)?;
        let key_type = field_type(&report, "keys")?;
        let StructuredInfoTypeShape::Collection { element, .. } = key_type.shape() else {
            return Err(refusal("hid-proof-type", "keys collection"));
        };
        let keys = (0..6)
            .map(|index| {
                StructuredInfoValue::leaf(
                    element.clone(),
                    vec![if pressed && index == 0 { 4 } else { 0 }],
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(value_error)?;
        let keys = StructuredInfoValue::collection(key_type.clone(), keys).map_err(value_error)?;
        let keyboard = StructuredInfoValue::variant(
            observed.clone(),
            "keyboard",
            record(
                &report,
                vec![
                    leaf(&report, "modifiers", vec![0])?,
                    StructuredFieldValue::new("keys", keys).map_err(value_error)?,
                ],
            )?,
        )
        .map_err(value_error)?;
        let change = record(
            &transition,
            vec![
                leaf(&transition, "usage", vec![4])?,
                leaf(&transition, "pressed", vec![u8::from(pressed)])?,
                leaf(&transition, "modifiers", vec![0])?,
            ],
        )?;
        for port in outputs {
            let value = match port.port_id.as_str() {
                "received" => &received,
                "observed" => &keyboard,
                "transition" => &change,
                _ => return Err(refusal("hid-proof-port", port.port_id.as_str())),
            };
            digest.update(sequence.to_le_bytes());
            digest.update(port.port_id.as_str().as_bytes());
            digest.update(value.canonical_bytes().map_err(value_error)?);
        }
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
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
            "usb-hid-keyboard-endpoint",
        )
        .unwrap();
        let definition = artifact.artifact().definition();
        let plan = &definition.internal_plan;
        let fragment = &plan.fragments[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let expected = expected_digest(&definition.external_capability.outputs).unwrap();
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
