//! Bind actual guest kernel evidence to the checked Source and observed machine.
use super::ConduitosError;
use conduit_core::bind_active_play;
use conduitos::usb_base::{
    control_contract::ControlContract,
    control_proof_plan::{self, ControlProofSubject},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct KernelProofSign {
    schema: String,
    proof_class: String,
    status: String,
    host_id: String,
    boot_id: String,
    controller_base_id: String,
    device_instance_id: String,
    source_document_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
    plan_id: String,
    fragment_id: String,
    active_play_id: String,
    transcript_digest: String,
    root_port: u8,
    slot: u8,
    attachment_epoch: u32,
    transfers: u16,
    short_transfers: u16,
    cycle_transitions: u16,
    initial_enqueue: usize,
    initial_cycle: u32,
    final_enqueue: usize,
    final_cycle: u32,
    output_capacity: usize,
    dma_bytes: u16,
    maximum_in_flight: u8,
    normal_close: bool,
    fixture_protocol: bool,
}

pub(super) fn extract(
    serial: &str,
    subject: &ControlProofSubject<'_>,
    initial: (usize, u32),
) -> Result<KernelProofSign, ConduitosError> {
    let refuse = |detail| ConduitosError::refusal("usb-control-kernel-proof-invalid", detail);
    let mut signs = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_USB_KERNEL_SIGN "));
    let one = signs
        .next()
        .ok_or_else(|| refuse("missing completed checked-kernel sign".to_string()))?;
    if signs.next().is_some() {
        return Err(refuse("duplicated checked-kernel sign".to_string()));
    }
    let sign: KernelProofSign =
        serde_json::from_str(one).map_err(|error| refuse(error.to_string()))?;
    let contract = ControlContract::prepare()
        .map_err(|error| refuse(format!("control contract: {error:?}")))?;
    let plan =
        control_proof_plan::plan(&contract, subject).map_err(|error| refuse(error.to_string()))?;
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let (mut cursor, mut cycle) = initial;
    let mut transitions = 0;
    for _ in 0..2 {
        if cursor > 28 {
            cursor = 0;
            cycle ^= 1;
            transitions += 1;
        }
        cursor += 3;
    }
    if sign.schema != "conduit.conduitos.usb-control-kernel/v1"
        || sign.proof_class != "freestanding-emulator"
        || sign.status != "completed"
        || sign.host_id != subject.host_id
        || sign.boot_id != subject.boot_id
        || sign.controller_base_id != subject.controller_base_id
        || sign.device_instance_id != subject.device_instance_id
        || sign.root_port != subject.root_port
        || sign.slot != subject.slot
        || sign.attachment_epoch != subject.attachment_epoch
        || sign.source_document_id != plan.source_document_id.as_str()
        || sign.checked_plot_id != plan.checked_plot_id.as_str()
        || sign.expanded_plot_id != plan.expanded_plot_id.as_str()
        || sign.plan_id != plan.plan_id.as_str()
        || sign.fragment_id != fragment.fragment_id.as_str()
        || sign.active_play_id != active.active_play_id.as_str()
        || sign.transfers != 2
        || sign.short_transfers != 2
        || sign.cycle_transitions != transitions
        || (sign.initial_enqueue, sign.initial_cycle) != initial
        || sign.final_enqueue != cursor
        || sign.final_cycle != cycle
        || sign.output_capacity != 4096
        || sign.dma_bytes != 8192
        || sign.maximum_in_flight != 1
        || !sign.normal_close
        || !sign.fixture_protocol
        || sign.transcript_digest.len() != 64
        || !sign
            .transcript_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || sign.transcript_digest.bytes().all(|byte| byte == b'0')
    {
        return Err(refuse(format!(
            "kernel sign differs from observed checked realization: {sign:?}"
        )));
    }
    Ok(sign)
}

#[cfg(test)]
mod tests {
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
        let contract = ControlContract::prepare().unwrap();
        let plan = control_proof_plan::plan(&contract, &subject).unwrap();
        let fragment = &plan.fragments[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        serde_json::json!({
            "schema":"conduit.conduitos.usb-control-kernel/v1", "proof_class":"freestanding-emulator", "status":"completed",
            "host_id":subject.host_id, "boot_id":subject.boot_id, "controller_base_id":subject.controller_base_id, "device_instance_id":subject.device_instance_id,
            "source_document_id":plan.source_document_id.as_str(), "checked_plot_id":plan.checked_plot_id.as_str(), "expanded_plot_id":plan.expanded_plot_id.as_str(),
            "plan_id":plan.plan_id.as_str(), "fragment_id":fragment.fragment_id.as_str(), "active_play_id":active.active_play_id.as_str(),
            "transcript_digest":"1".repeat(64), "root_port":1, "slot":1, "attachment_epoch":1,
            "transfers":2, "short_transfers":2, "cycle_transitions":1, "initial_enqueue":27, "initial_cycle":1,
            "final_enqueue":3, "final_cycle":0, "output_capacity":4096, "dma_bytes":8192, "maximum_in_flight":1, "normal_close":true, "fixture_protocol":true
        })
    }
    fn serial(value: &serde_json::Value) -> String {
        format!("CONDUIT_USB_KERNEL_SIGN {value}\n")
    }
    #[test]
    fn receipt_binds_checked_identity_machine_and_finite_storage() {
        let valid = fixture();
        assert!(extract(&serial(&valid), &subject(), (27, 1)).is_ok());
        for field in [
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
            "short_transfers",
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
        for field in ["normal_close", "fixture_protocol"] {
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
}
