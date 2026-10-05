
use super::*;
fn subject() -> ControlProofSubject<'static> {
    ControlProofSubject {
        host_id: "host/proof",
        boot_id: "boot/proof",
        controller_base_id: "base/proof",
        device_instance_id: "device/proof",
        root_port: 1,
        slot: 1,
        attachment_epoch: 1,
    }
}
fn fixture() -> serde_json::Value {
    let subject = subject();
    let artifact = device_probe_proof_plan::prepare(&subject).unwrap();
    let plan = &artifact.artifact().definition().internal_plan;
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    serde_json::json!({
        "schema":"conduit.conduitos.usb-device-probe/v1", "proof_class":"freestanding-emulator", "status":"completed",
        "host_id":subject.host_id, "boot_id":subject.boot_id, "controller_base_id":subject.controller_base_id, "device_instance_id":subject.device_instance_id,
        "source_document_id":plan.source_document_id.as_str(), "checked_plot_id":plan.checked_plot_id.as_str(), "expanded_plot_id":plan.expanded_plot_id.as_str(),
        "plan_id":plan.plan_id.as_str(), "fragment_id":fragment.fragment_id.as_str(), "active_play_id":active.active_play_id.as_str(),
        "transcript_digest":"1".repeat(64), "root_port":1, "slot":1, "attachment_epoch":1,
        "transfers":64, "decoded":64, "observed":64, "additional_local_sign_items":4096, "additional_remote_sign_items":512, "cycle_transitions":7, "initial_enqueue":27, "initial_cycle":1,
        "final_enqueue":9, "final_cycle":0, "output_capacity":4096, "dma_bytes":8192, "maximum_in_flight":1, "normal_close":true, "protocol_owned_by_source":true, "legacy_attachment_setup":true, "fixture_appliance":true
    })
}
fn serial(value: &serde_json::Value) -> String {
    format!("CONDUIT_USB_DEVICE_PROBE_SIGN {value}\n")
}
#[test]
fn receipt_binds_checked_identity_machine_and_finite_storage() {
    let valid = fixture();
    assert!(extract(&serial(&valid), &subject(), (27, 1)).is_ok());
    for field in [
        "schema",
        "host_id",
        "boot_id",
        "controller_base_id",
        "device_instance_id",
        "source_document_id",
        "checked_plot_id",
        "expanded_plot_id",
        "plan_id",
        "fragment_id",
        "active_play_id",
        "proof_class",
        "status",
        "transcript_digest",
    ] {
        let mut changed = valid.clone();
        changed[field] = "stale".into();
        assert!(
            extract(&serial(&changed), &subject(), (27, 1)).is_err(),
            "{field}"
        );
    }
    for field in [
        "root_port",
        "slot",
        "attachment_epoch",
        "transfers",
        "decoded",
        "observed",
        "initial_enqueue",
        "initial_cycle",
        "additional_local_sign_items",
        "additional_remote_sign_items",
        "cycle_transitions",
        "final_enqueue",
        "final_cycle",
        "output_capacity",
        "dma_bytes",
        "maximum_in_flight",
    ] {
        let mut changed = valid.clone();
        changed[field] = 99.into();
        assert!(
            extract(&serial(&changed), &subject(), (27, 1)).is_err(),
            "{field}"
        );
    }
    for field in [
        "normal_close",
        "protocol_owned_by_source",
        "legacy_attachment_setup",
        "fixture_appliance",
    ] {
        let mut changed = valid.clone();
        changed[field] = false.into();
        assert!(
            extract(&serial(&changed), &subject(), (27, 1)).is_err(),
            "{field}"
        );
    }
    assert!(extract(&serial(&valid), &subject(), (24, 1)).is_err());
}
#[test]
fn receipt_refuses_missing_duplicate_and_unknown_fields() {
    let valid = serial(&fixture());
    assert!(extract("", &subject(), (27, 1)).is_err());
    assert!(extract(&format!("{valid}{valid}"), &subject(), (27, 1)).is_err());
    let mut unknown = fixture();
    unknown["invented_authority"] = true.into();
    assert!(extract(&serial(&unknown), &subject(), (27, 1)).is_err());
}
