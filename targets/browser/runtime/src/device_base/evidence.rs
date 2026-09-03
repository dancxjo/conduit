use super::abi::AbiState;
use super::{
    BrowserSerialPhase, SERIAL_ACQUIRE_OPERATION, SERIAL_ACQUISITION_CAPABILITY,
    SERIAL_REQUEST_AUTHORITY,
};
use conduit_core::{CapabilityId, PlanId};

pub(super) fn refresh_evidence(state: &mut AbiState) {
    let (phase, terminal) = match state.session.phase() {
        BrowserSerialPhase::OfferAvailable => ("offer-available", None),
        BrowserSerialPhase::AcquisitionPlanned { .. } => ("acquisition-plan-sealed", None),
        BrowserSerialPhase::AcquisitionPlaying { .. } => ("acquisition-play-started", None),
        BrowserSerialPhase::ResourceTruth(_) => ("resource-truth", None),
        BrowserSerialPhase::UsePlanned { .. } => ("serial-use-plan-sealed", None),
        BrowserSerialPhase::UsePlaying { .. } => ("serial-use-playing", None),
        BrowserSerialPhase::Terminal(value) => ("terminal", Some(format!("{value:?}"))),
    };
    let all_stages = [
        (0b0000_0001, "offer-available"),
        (0b0000_0010, "acquisition-plan-sealed"),
        (0b0000_0100, "acquisition-play-started"),
        (0b0000_1000, "browser-result"),
        (0b0001_0000, "resource-truth-entered"),
        (0b0010_0000, "serial-use-plan-sealed"),
        (0b0100_0000, "serial-use-playing"),
        (0b1000_0000, "bounded-transfer-observed"),
    ];
    let stages = all_stages
        .into_iter()
        .filter_map(|(bit, name)| (state.stages & bit != 0).then_some(name))
        .collect::<Vec<_>>();
    let resource = state.resource.as_ref();
    let current_device = state
        .session
        .current_device_association(vec![CapabilityId::from(SERIAL_ACQUISITION_CAPABILITY)]);
    let value = serde_json::json!({
        "schema": "conduit.browser/web-serial-base-evidence@1",
        "host_id": state.host_id.as_str(), "boot_id": state.boot_id.as_str(),
        "operation_id": state.operation_id.as_str(), "phase": phase, "terminal": terminal,
        "acquisition_plan_id": state.acquisition_plan_id.as_str(),
        "use_plan_id": state.use_plan_id.as_ref().map(PlanId::as_str),
        "operation_contract": SERIAL_ACQUIRE_OPERATION,
        "request_authority_contract": SERIAL_REQUEST_AUTHORITY,
        "permission": "explicit-user-action-required",
        "stages": stages,
        "resource_handle": resource.map(|value| value.handle_id.as_str()),
        "resource_class": resource.map(|value| value.class_id.as_str()),
        "base_implementation_id": resource.map(|value| value.base_implementation_id.as_str()),
        "base_instance_id": resource.map(|value| value.base_instance_id.as_str()),
        "use_authority_contract": resource.map(|value| value.use_authority_contract.as_str()),
        "use_authority_grant": resource.map(|value| value.use_authority_grant.as_str()),
        "usb_vendor_id": resource.and_then(|value| value.usb_vendor_id),
        "usb_product_id": resource.and_then(|value| value.usb_product_id),
        "current_device": current_device,
        "configuration": {
            "baud_rate": state.configuration.baud_rate,
            "data_bits": state.configuration.data_bits,
            "stop_bits": state.configuration.stop_bits,
            "parity": format!("{:?}", state.configuration.parity),
            "buffer_size": state.configuration.buffer_size,
        },
        "transfer_bounds": {
            "maximum_transfer_bytes": state.transfer_bounds.maximum_transfer_bytes,
            "maximum_reads": state.transfer_bounds.maximum_reads,
            "maximum_writes": state.transfer_bounds.maximum_writes,
            "maximum_signal_operations": state.transfer_bounds.maximum_signal_operations,
            "maximum_in_flight": state.transfer_bounds.maximum_in_flight,
        },
        "admitted_reads": state.session.admitted_reads(),
        "admitted_writes": state.session.admitted_writes(),
        "admitted_signal_operations": state.session.admitted_signal_operations(),
        "retained_bytes": state.session.retained_bytes(),
        "last_transfer_direction": state.last_transfer_direction.map(|value| format!("{value:?}")),
        "last_transfer_bytes": state.last_transfer_bytes,
        "last_transfer_checksum": state.last_transfer_checksum,
    });
    if let Ok(encoded) = serde_json::to_vec(&value) {
        if encoded.len() <= state.evidence.len() {
            state.evidence[..encoded.len()].copy_from_slice(&encoded);
            state.evidence_len = encoded.len();
        }
    }
}
