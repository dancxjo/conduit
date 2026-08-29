use super::abi::AbiState;
use super::{BrowserUsbPhase, USB_ACQUIRE_OPERATION, USB_REQUEST_AUTHORITY};
use conduit_core::PlanId;

pub(super) fn refresh_evidence(state: &mut AbiState) {
    let (phase, terminal) = match state.session.phase() {
        BrowserUsbPhase::OfferAvailable => ("offer-available", None),
        BrowserUsbPhase::AcquisitionPlanned { .. } => ("acquisition-plan-sealed", None),
        BrowserUsbPhase::AcquisitionPlaying { .. } => ("acquisition-play-started", None),
        BrowserUsbPhase::ResourceTruth(_) => ("resource-truth", None),
        BrowserUsbPhase::UsePlanned { .. } => ("usb-use-plan-sealed", None),
        BrowserUsbPhase::UsePlaying { .. } => ("usb-use-playing", None),
        BrowserUsbPhase::Terminal(value) => ("terminal", Some(format!("{value:?}"))),
    };
    let all_stages = [
        (0b0000_0001, "offer-available"),
        (0b0000_0010, "acquisition-plan-sealed"),
        (0b0000_0100, "acquisition-play-started"),
        (0b0000_1000, "browser-result"),
        (0b0001_0000, "resource-truth-entered"),
        (0b0010_0000, "usb-use-plan-sealed"),
        (0b0100_0000, "usb-use-playing"),
        (0b1000_0000, "bounded-transfer-observed"),
    ];
    let stages = all_stages
        .into_iter()
        .filter_map(|(bit, name)| (state.stages & bit != 0).then_some(name))
        .collect::<Vec<_>>();
    let resource = state.resource.as_ref();
    let value = serde_json::json!({
        "schema": "conduit.browser/web-usb-base-evidence@1",
        "host_id": state.host_id.as_str(), "boot_id": state.boot_id.as_str(),
        "operation_id": state.operation_id.as_str(), "phase": phase, "terminal": terminal,
        "acquisition_plan_id": state.acquisition_plan_id.as_str(),
        "use_plan_id": state.use_plan_id.as_ref().map(PlanId::as_str),
        "operation_contract": USB_ACQUIRE_OPERATION,
        "request_authority_contract": USB_REQUEST_AUTHORITY,
        "permission": "explicit-user-action-required",
        "stages": stages,
        "resource_handle": resource.map(|value| value.handle_id.as_str()),
        "resource_class": resource.map(|value| value.class_id.as_str()),
        "base_implementation_id": resource.map(|value| value.base_implementation_id.as_str()),
        "base_instance_id": resource.map(|value| value.base_instance_id.as_str()),
        "use_authority_contract": resource.map(|value| value.use_authority_contract.as_str()),
        "use_authority_grant": resource.map(|value| value.use_authority_grant.as_str()),
        "vendor_id": resource.map(|value| value.vendor_id),
        "product_id": resource.map(|value| value.product_id),
        "configuration": {
            "configuration_value": state.configuration.configuration_value,
            "interface_number": state.configuration.interface_number,
            "alternate_setting": state.configuration.alternate_setting,
            "in_endpoint": state.configuration.in_endpoint,
            "out_endpoint": state.configuration.out_endpoint,
        },
        "transfer_bounds": {
            "maximum_transfer_bytes": state.transfer_bounds.maximum_transfer_bytes,
            "maximum_in_transfers": state.transfer_bounds.maximum_in_transfers,
            "maximum_out_transfers": state.transfer_bounds.maximum_out_transfers,
            "maximum_in_flight": state.transfer_bounds.maximum_in_flight,
        },
        "admitted_in_transfers": state.session.admitted_in_transfers(),
        "admitted_out_transfers": state.session.admitted_out_transfers(),
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
