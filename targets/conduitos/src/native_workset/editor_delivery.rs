//! Exact canonical native editor encounter over the initialized keyboard delivery.
use super::WorksetRefusal;
use alloc::{format, vec};
use conduit_core::{
    Back, BackOfferBuilder, CapabilityOffer, HostCallRequirement, PlanFragment, PlannedGear,
    resource_requirement,
};

pub(crate) const IMPLEMENTATION: &str = "conduitos/native-document-editor@1";
pub(crate) const HOST_CALL: &str = "conduit.host/conduitos-document-editor@1";

pub(super) fn offer(build: &str) -> CapabilityOffer {
    let mut offer = BackOfferBuilder::new(
        conduit_semantic_catalog::face_editor_semantic_contract(),
        Back {
            capability_id: IMPLEMENTATION.into(),
            execution_profile_id: "conduitos/bounded-document-encounter@1".into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: format!("conduitos-build/{build}").into(),
            host_calls: vec![HostCallRequirement {
                contract_id: HOST_CALL.into(),
                target_kind: Some("face/native-editor@1".into()),
                maximum_in_flight: 1,
                maximum_input_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES,
                maximum_output_bytes: conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES,
            }],
            resource_requirements: vec![
                resource_requirement(super::keyboard_delivery::RESOURCE, 1),
                resource_requirement(
                    "conduit.resource/runtime-memory@1",
                    (core::mem::size_of::<super::NativeWorksetPlay>() + 512) as u32,
                ),
            ],
            authority_requirements: vec![],
        },
    )
    .build();
    offer.resource_requirements.sort();
    offer
}

pub(super) fn validate(placement: &PlannedGear) -> Result<(), WorksetRefusal> {
    let expected = offer("");
    if placement.kind_id != expected.kind_id
        || placement.capability_id != expected.capability_id
        || placement.kind_contract_revision != expected.kind_contract_revision
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.host_calls != expected.host_calls
        || placement.limits != expected.limits
        || placement.semantic_contract != expected.semantic_contract
        || !placement.authority.is_empty()
        || placement.configuration.len() != 1
    {
        return Err(WorksetRefusal::Plan);
    }
    conduit_semantic_catalog::validate_declared_data_labels(
        &placement.configuration,
        &expected.semantic_contract.configuration,
    )
    .map_err(|_| WorksetRefusal::Plan)?;
    conduit_semantic_catalog::face_data_label(&placement.configuration)
        .map_err(|_| WorksetRefusal::Plan)?;
    Ok(())
}

/// The two-node initialized loop is the only additional admitted topology. All
/// existing three/four-node profiles retain their previous admission boundary.
pub(super) fn partition_shape(fragment: &PlanFragment) -> Result<bool, WorksetRefusal> {
    let canonical = fragment.placements.iter().any(|placement| {
        matches!(
            placement.kind_id.as_str(),
            conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
                | conduit_semantic_catalog::FACE_EDITOR_KIND
        )
    });
    if !canonical {
        return Ok(matches!(fragment.placements.len(), 3 | 4));
    }
    if fragment.placements.len() != 2
        || fragment.connections.len() != 2
        || !fragment.fore_ports.is_empty()
    {
        return Ok(false);
    }
    let Some(document) = fragment.placements.iter().find(|placement| {
        placement.kind_id.as_str() == conduit_semantic_catalog::TEXT_EDIT_DOCUMENT_KIND
    }) else {
        return Ok(false);
    };
    let Some(editor) = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::FACE_EDITOR_KIND)
    else {
        return Ok(false);
    };
    super::document_state::validate(document)?;
    validate(editor)?;
    let mut current = false;
    let mut edit = false;
    for cord in &fragment.connections {
        if cord.track != conduit_core::ConnectionTrack::Payload
            || cord.temporal != (conduit_core::PortTemporal::Flow { closes: false })
            || cord.resource.is_some()
            || cord.abnormal_kind.is_some()
            || cord.selected_line.is_some()
            || !cord.admitted_lines.is_empty()
            || cord.item_capacity != 1
        {
            return Ok(false);
        }
        let minimum = if cord.source_placement_id == document.placement_id
            && cord.source_port_id.as_str() == "current"
            && cord.sink_placement_id == editor.placement_id
            && cord.sink_port_id.as_str() == "document"
            && cord.value_kind.as_str() == conduit_semantic_catalog::TEXT_DOCUMENT_VALUE_KIND
            && !current
        {
            current = true;
            conduit_semantic_catalog::MAXIMUM_TEXT_DOCUMENT_BYTES
        } else if cord.source_placement_id == editor.placement_id
            && cord.source_port_id.as_str() == "edit"
            && cord.sink_placement_id == document.placement_id
            && cord.sink_port_id.as_str() == "edit"
            && cord.value_kind.as_str() == conduit_semantic_catalog::TEXT_EDIT_VALUE_KIND
            && !edit
        {
            edit = true;
            conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES
        } else {
            return Ok(false);
        };
        if !(minimum..=conduit_semantic_catalog::MAXIMUM_TEXT_EDIT_BYTES)
            .contains(&cord.byte_capacity)
        {
            return Ok(false);
        }
    }
    Ok(current && edit)
}
