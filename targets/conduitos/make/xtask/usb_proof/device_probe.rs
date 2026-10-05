//! Bind actual guest kernel evidence to the checked Source and observed machine.
use super::ConduitosError;
use conduit_core::bind_active_play;
use conduitos::usb_base::{
    control_proof_plan::{self, ControlProofSubject},
    device_probe_proof_plan,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DeviceProbeSign {
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
    decoded: u16,
    observed: u16,
    additional_local_sign_items: u16,
    additional_remote_sign_items: u16,
    cycle_transitions: u16,
    initial_enqueue: usize,
    initial_cycle: u32,
    final_enqueue: usize,
    final_cycle: u32,
    output_capacity: usize,
    dma_bytes: u16,
    maximum_in_flight: u8,
    normal_close: bool,
    protocol_owned_by_source: bool,
    legacy_attachment_setup: bool,
    fixture_appliance: bool,
}

pub(super) fn extract(
    serial: &str,
    subject: &ControlProofSubject<'_>,
    initial: (usize, u32),
) -> Result<DeviceProbeSign, ConduitosError> {
    let refuse = |detail| ConduitosError::refusal("usb-device-probe-proof-invalid", detail);
    let mut signs = serial
        .lines()
        .filter_map(|line| line.strip_prefix("CONDUIT_USB_DEVICE_PROBE_SIGN "));
    let one = signs
        .next()
        .ok_or_else(|| refuse("missing completed checked-kernel sign".to_string()))?;
    if signs.next().is_some() {
        return Err(refuse("duplicated checked-kernel sign".to_string()));
    }
    let sign: DeviceProbeSign =
        serde_json::from_str(one).map_err(|error| refuse(error.to_string()))?;
    let artifact = device_probe_proof_plan::prepare(subject)
        .map_err(|error| refuse(format!("device Source planning: {error:?}")))?;
    let plan = &artifact.artifact().definition().internal_plan;
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let (mut cursor, mut cycle) = initial;
    let mut transitions = 0;
    for _ in 0..control_proof_plan::CONTROL_PROOF_TRANSFERS {
        if cursor > 28 {
            cursor = 0;
            cycle ^= 1;
            transitions += 1;
        }
        cursor += 3;
    }
    if sign.schema != "conduit.conduitos.usb-device-probe/v1"
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
        || sign.transfers != control_proof_plan::CONTROL_PROOF_TRANSFERS
        || sign.decoded != control_proof_plan::CONTROL_PROOF_TRANSFERS
        || sign.observed != control_proof_plan::CONTROL_PROOF_TRANSFERS
        || sign.additional_local_sign_items != 4096
        || sign.additional_remote_sign_items != 512
        || sign.cycle_transitions < 4
        || sign.cycle_transitions != transitions
        || (sign.initial_enqueue, sign.initial_cycle) != initial
        || sign.final_enqueue != cursor
        || sign.final_cycle != cycle
        || sign.output_capacity != 4096
        || sign.dma_bytes != 8192
        || sign.maximum_in_flight != 1
        || !sign.normal_close
        || !sign.protocol_owned_by_source
        || !sign.legacy_attachment_setup
        || !sign.fixture_appliance
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
mod tests;
