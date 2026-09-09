//! Native lifecycle evidence emitted from the authoritative journey projection.
use crate::{arch, fabrication::FabricationRecord, product_journey::JourneyProjection};
use alloc::{format, string::String};

pub(super) fn emit_journey_sign(
    projection: &JourneyProjection,
    fabrication: &FabricationRecord,
    receipt: &crate::native_compositor::CompositionReceipt,
) {
    let line = format!(
        "CONDUIT_PRODUCT_JOURNEY {{\"status\":\"{}\",\"revision\":{},\"profile_id\":\"{}\",\"build_id\":\"{}\",\"image_id\":\"{}\",\"host_id\":\"{}\",\"boot_id\":\"{}\",\"offer_generation\":{},\"source_document_id\":\"{}\",\"checked_form_id\":\"{}\",\"expanded_form_id\":\"{}\",\"body_id\":{},\"friendly_name\":{},\"born_sign_id\":{},\"part_id\":{},\"wake_id\":{},\"plan_id\":{},\"active_play_id\":{},\"gear_ids\":{},\"port_ids\":{},\"cord_ids\":{},\"presentation_id\":\"{}\",\"manifestation_id\":\"{}\",\"presenter_implementation_id\":\"{}\",\"input_sign_id\":{},\"result_sign_id\":{},\"result\":{},\"request_id\":{}}}\n",
        projection.status.as_str(),
        projection.revision,
        fabrication.profile_id,
        fabrication.build_id,
        fabrication.image_binding,
        projection.host_id.as_str(),
        projection.boot_id.as_str(),
        projection.offer_generation.0,
        projection.source_document_id.as_str(),
        projection.checked_form_id.as_str(),
        projection.expanded_form_id.as_str(),
        json_identity(
            projection
                .body_id
                .as_ref()
                .map(conduit_body::BodyId::as_str)
        ),
        json_identity(projection.friendly_name.as_deref()),
        json_identity(
            projection
                .born_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(
            projection
                .part_id
                .as_ref()
                .map(conduit_body::PartId::as_str)
        ),
        json_identity(
            projection
                .wake_id
                .as_ref()
                .map(conduit_body::WakeId::as_str)
        ),
        json_identity(
            projection
                .plan_id
                .as_ref()
                .map(conduit_core::PlanId::as_str)
        ),
        json_identity(
            projection
                .active_play_id
                .as_ref()
                .map(conduit_core::ActivePlayId::as_str)
        ),
        json_array(&projection.gear_ids),
        json_array(&projection.port_ids),
        json_array(&projection.cord_ids),
        receipt.presentation_id.as_str(),
        receipt.manifestation_id.as_str(),
        receipt.presenter_implementation_id.as_str(),
        json_identity(
            projection
                .input_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(
            projection
                .result_sign_id
                .as_ref()
                .map(conduit_core::SignId::as_str)
        ),
        json_identity(projection.result.as_deref()),
        json_identity(projection.last_request_id.as_deref()),
    );
    arch::early_write(line.as_bytes());
}

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<alloc::vec::Vec<_>>()
            .join(",")
    )
}

fn json_identity(value: Option<&str>) -> String {
    serde_json::to_string(&value).expect("finite optional text serializes")
}
