use conduit_core::{
    AcquiredMediaResource, MediaAcquisitionAuthority, MediaAcquisitionOffer, MediaAcquisitionPlan,
    MediaAcquisitionRequest, MediaAcquisitionReservation, MediaPlanningRefusal,
    MediaResourceAvailability, MediaUseRequirement, PlanId, SelectedMediaResource,
};

pub fn plan_media_acquisition(
    plan_id: PlanId,
    offer: &MediaAcquisitionOffer,
    authority: Option<&MediaAcquisitionAuthority>,
    request: MediaAcquisitionRequest,
    occupied_operation_slots: u16,
) -> Result<MediaAcquisitionPlan, MediaPlanningRefusal> {
    if !request.constraints.is_valid() {
        return Err(MediaPlanningRefusal::InvalidConstraints);
    }
    if !request.flow_bounds.is_finite_and_valid() {
        return Err(MediaPlanningRefusal::InvalidBounds);
    }
    if offer.maximum_in_flight == 0 || offer.maximum_result_bytes == 0 {
        return Err(MediaPlanningRefusal::OfferUnavailable);
    }
    let authority = authority.ok_or(MediaPlanningRefusal::RequestAuthorityMissing)?;
    if authority.contract_id != offer.request_authority_contract
        || authority.host_id != offer.host_id
        || authority.boot_id != offer.boot_id
        || authority.kind != offer.kind
        || request.constraints.kind() != offer.kind
    {
        return Err(MediaPlanningRefusal::RequestAuthorityMismatch);
    }
    if occupied_operation_slots >= offer.maximum_in_flight {
        return Err(MediaPlanningRefusal::CapacityExhausted);
    }
    Ok(MediaAcquisitionPlan {
        plan_id,
        host_id: offer.host_id.clone(),
        boot_id: offer.boot_id.clone(),
        offer_generation: offer.offer_generation,
        operation_contract: offer.operation_contract.clone(),
        request_authority_grant: authority.grant_id.clone(),
        reservation: MediaAcquisitionReservation {
            operation_id: request.operation_id.clone(),
            slot: occupied_operation_slots,
            maximum_result_bytes: offer.maximum_result_bytes,
        },
        request,
    })
}

pub fn select_acquired_media(
    requirement: &MediaUseRequirement,
    resource: &AcquiredMediaResource,
    use_authority_grant: Option<&conduit_core::AuthorityGrantId>,
) -> Result<SelectedMediaResource, MediaPlanningRefusal> {
    match resource.availability {
        MediaResourceAvailability::Lost => return Err(MediaPlanningRefusal::ResourceLost),
        MediaResourceAvailability::Closed => return Err(MediaPlanningRefusal::ResourceClosed),
        MediaResourceAvailability::Available => {}
    }
    let grant = use_authority_grant.ok_or(MediaPlanningRefusal::UseAuthorityMissing)?;
    if *grant != resource.use_authority_grant {
        return Err(MediaPlanningRefusal::UseAuthorityMissing);
    }
    if requirement.kind != resource.settings.kind()
        || requirement.output_port.as_str().is_empty()
        || requirement.class_id != resource.class_id
        || requirement.value_kind != resource.value_kind
    {
        return Err(MediaPlanningRefusal::WrongResourceKind);
    }
    if !requirement.flow_bounds.is_finite_and_valid()
        || requirement.flow_bounds.maximum_value_bytes > resource.flow_bounds.maximum_value_bytes
        || requirement.flow_bounds.maximum_queue_items > resource.flow_bounds.maximum_queue_items
        || requirement.flow_bounds.maximum_queue_bytes > resource.flow_bounds.maximum_queue_bytes
    {
        return Err(MediaPlanningRefusal::BoundsUnsatisfied);
    }
    Ok(SelectedMediaResource {
        output_port: requirement.output_port.clone(),
        handle_id: resource.handle_id.clone(),
        use_authority_grant: grant.clone(),
        host_id: resource.host_id.clone(),
        boot_id: resource.boot_id.clone(),
        flow_bounds: requirement.flow_bounds,
    })
}
