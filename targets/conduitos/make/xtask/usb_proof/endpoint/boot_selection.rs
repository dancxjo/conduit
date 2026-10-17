//! Independently reconstruct the finite class exchange and its exact Plan.
use super::{extract, refusal, ConduitosError, GuestBootSign, GuestUsbSign};
use conduit_core::{
    bind_active_play, StructuredFieldValue, StructuredInfoTypeShape, StructuredInfoValue,
};
use conduitos::usb_base::{
    control_contract::ControlContract, control_proof_plan::ControlProofSubject,
    control_request::ControlTransferRequest, control_result::PreparedControlResultEncoder,
    hid_boot_control_proof_plan,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Sign {
    schema: String,
    proof_class: String,
    status: String,
    host_id: String,
    boot_id: String,
    controller_base_id: String,
    device_instance_id: String,
    source_document_id: String,
    checked_plot_id: String,
    plan_id: String,
    active_play_id: String,
    attachment_epoch: u32,
    interface_number: u8,
    transfers: u16,
    transcript_digest: String,
    normal_close: bool,
    quiescent_release: bool,
    fixture_appliance: bool,
    ordinary_class_offer: bool,
}

pub(super) fn verify(
    serial: &str,
    boot: &GuestBootSign,
    usb: &GuestUsbSign,
) -> Result<Sign, ConduitosError> {
    let sign: Sign = extract(serial, "CONDUIT_USB_HID_BOOT_SIGN ")?;
    let expected = expected(
        &ControlProofSubject {
            host_id: &boot.host_id,
            boot_id: &boot.boot_id,
            controller_base_id: &usb.controller_base_id,
            device_instance_id: &usb.device_instance_id,
            root_port: usb.root_port,
            slot: usb.slot,
            attachment_epoch: usb.attachment_epoch,
        },
        usb.first_interface_number,
    )?;
    validate(&sign, &expected)?;
    Ok(sign)
}

fn validate(sign: &Sign, expected: &Sign) -> Result<(), ConduitosError> {
    if sign != expected {
        return Err(refusal(
            "hid-boot-proof-invalid-receipt",
            format!("{sign:?}"),
        ));
    }
    Ok(())
}

fn expected(subject: &ControlProofSubject<'_>, interface: u8) -> Result<Sign, ConduitosError> {
    let artifact = hid_boot_control_proof_plan::prepare(subject)
        .map_err(|error| refusal("hid-boot-proof-plan", format!("{error:?}")))?;
    let plan = &artifact.artifact().definition().internal_plan;
    let active = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        0,
    );
    Ok(Sign {
        schema: "conduit.conduitos.usb-hid-boot-selection/v1".into(),
        proof_class: "freestanding-emulator".into(),
        status: "ready".into(),
        host_id: subject.host_id.into(),
        boot_id: subject.boot_id.into(),
        controller_base_id: subject.controller_base_id.into(),
        device_instance_id: subject.device_instance_id.into(),
        source_document_id: plan.source_document_id.as_str().into(),
        checked_plot_id: plan.checked_plot_id.as_str().into(),
        plan_id: plan.plan_id.as_str().into(),
        active_play_id: active.active_play_id.as_str().into(),
        attachment_epoch: subject.attachment_epoch,
        interface_number: interface,
        transfers: 1,
        transcript_digest: expected_transcript(interface)?,
        normal_close: true,
        quiescent_release: true,
        fixture_appliance: true,
        ordinary_class_offer: false,
    })
}

// The wire expectation comes from the class contract, without evaluating the
// Source expression that constructs the actual native request.
fn expected_transcript(interface: u8) -> Result<String, ConduitosError> {
    let fail = |error| refusal("hid-boot-proof-contract", format!("{error:?}"));
    let contract = ControlContract::prepare().map_err(fail)?;
    let StructuredInfoTypeShape::Record { fields, .. } = contract.request_type().shape() else {
        return Err(refusal("hid-boot-proof-contract", "request record"));
    };
    let ty = |name| {
        fields
            .iter()
            .find(|field| field.name() == name)
            .map(|field| field.value_type().clone())
            .ok_or_else(|| refusal("hid-boot-proof-contract", "request field"))
    };
    let setup = [0x21, 11, 0, 0, interface, 0, 0, 0];
    let request = StructuredInfoValue::record(
        contract.request_type().clone(),
        vec![
            StructuredFieldValue::new(
                "setup",
                StructuredInfoValue::leaf(ty("setup")?, setup.to_vec()).map_err(fail)?,
            )
            .map_err(fail)?,
            StructuredFieldValue::new(
                "output",
                StructuredInfoValue::sequence(ty("output")?, vec![]).map_err(fail)?,
            )
            .map_err(fail)?,
        ],
    )
    .map_err(fail)?
    .canonical_bytes()
    .map_err(fail)?;
    let raw = ControlTransferRequest::new(setup, &[], 256)
        .map_err(|error| refusal("hid-boot-proof-contract", format!("{error:?}")))?;
    let mut encoder = PreparedControlResultEncoder::new(&contract).map_err(fail)?;
    let result = encoder
        .completed(&raw, 0, &[])
        .map_err(|error| refusal("hid-boot-proof-contract", format!("{error:?}")))?;
    let mut digest = Sha256::new();
    digest.update(request);
    digest.update(result);
    Ok(conduitos::identity::hex(&digest.finalize().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boot_selection_receipt_rejects_substitution_and_missing_lifecycle() {
        let expected = expected(
            &ControlProofSubject {
                host_id: "host/boot-test",
                boot_id: "boot/boot-test",
                controller_base_id: "base/boot-test",
                device_instance_id: "device/boot-test",
                root_port: 1,
                slot: 1,
                attachment_epoch: 1,
            },
            7,
        )
        .unwrap();
        validate(&expected, &expected).unwrap();
        let original = serde_json::to_value(&expected).unwrap();
        for (field, changed) in [
            ("schema", serde_json::json!("foreign")),
            ("proof_class", serde_json::json!("simulation")),
            ("status", serde_json::json!("stalled")),
            ("host_id", serde_json::json!("foreign")),
            ("boot_id", serde_json::json!("stale")),
            ("controller_base_id", serde_json::json!("foreign")),
            ("device_instance_id", serde_json::json!("foreign")),
            ("source_document_id", serde_json::json!("foreign")),
            ("checked_plot_id", serde_json::json!("foreign")),
            ("plan_id", serde_json::json!("foreign")),
            ("active_play_id", serde_json::json!("foreign")),
            ("attachment_epoch", serde_json::json!(2)),
            ("interface_number", serde_json::json!(8)),
            ("transfers", serde_json::json!(2)),
            ("transcript_digest", serde_json::json!("foreign")),
            ("normal_close", serde_json::json!(false)),
            ("quiescent_release", serde_json::json!(false)),
            ("fixture_appliance", serde_json::json!(false)),
            ("ordinary_class_offer", serde_json::json!(true)),
        ] {
            let mut altered = original.clone();
            altered[field] = changed;
            let sign: Sign = serde_json::from_value(altered).unwrap();
            assert!(
                validate(&sign, &expected).is_err(),
                "accepted changed {field}"
            );
        }
        assert_ne!(
            expected_transcript(7).unwrap(),
            expected_transcript(8).unwrap()
        );
    }
}
