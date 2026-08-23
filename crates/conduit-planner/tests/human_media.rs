use conduit_core::{
    AcquiredMediaResource, AuthorityContractId, AuthorityGrantId, BootId, HostId,
    HostOperationContractId, HostOperationId, HumanMediaKind, KindId, KnownPermissionState,
    MediaAcquisitionAuthority, MediaAcquisitionOffer, MediaAcquisitionRequest,
    MediaAcquisitionResult, MediaConstraints, MediaFlowBounds, MediaPlanningRefusal,
    MediaResourceAvailability, MediaUseRequirement, OfferGeneration, PlanId, PortId,
    ResourceClassId, ResourceHandleId,
};
use conduit_planner::{plan_media_acquisition, select_acquired_media};

fn bounds() -> MediaFlowBounds {
    MediaFlowBounds {
        maximum_value_bytes: 4096,
        maximum_queue_items: 4,
        maximum_queue_bytes: 16384,
    }
}

fn camera_constraints() -> MediaConstraints {
    MediaConstraints::Camera {
        minimum_width: 320,
        maximum_width: 1280,
        minimum_height: 240,
        maximum_height: 720,
        maximum_frames_per_second: 30,
    }
}

fn offer() -> MediaAcquisitionOffer {
    MediaAcquisitionOffer {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        offer_generation: OfferGeneration(3),
        kind: HumanMediaKind::Camera,
        operation_contract: HostOperationContractId::from("conduit.host/acquire-human-media@1"),
        request_authority_contract: AuthorityContractId::from(
            "conduit.authority/request-human-media@1",
        ),
        known_permission: KnownPermissionState::Prompt,
        maximum_in_flight: 1,
        maximum_result_bytes: 1024,
    }
}

fn authority() -> MediaAcquisitionAuthority {
    MediaAcquisitionAuthority {
        grant_id: AuthorityGrantId::from("grant/request-camera"),
        contract_id: AuthorityContractId::from("conduit.authority/request-human-media@1"),
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        kind: HumanMediaKind::Camera,
    }
}

fn request() -> MediaAcquisitionRequest {
    MediaAcquisitionRequest {
        operation_id: HostOperationId::from("operation/acquire-camera-1"),
        constraints: camera_constraints(),
        flow_bounds: bounds(),
    }
}

#[test]
fn acquisition_plan_is_immutable_evidence_and_use_requires_new_resource_truth() {
    let acquisition = plan_media_acquisition(
        PlanId::from("plan/acquire-camera"),
        &offer(),
        Some(&authority()),
        request(),
        0,
    )
    .unwrap();
    let sealed_before_result = acquisition.clone();
    let resource = AcquiredMediaResource {
        host_id: acquisition.host_id.clone(),
        boot_id: acquisition.boot_id.clone(),
        handle_id: ResourceHandleId::from("opaque-track/7"),
        class_id: ResourceClassId::from("conduit.resource/acquired-camera@1"),
        value_kind: KindId::from("media/camera-frame@1"),
        settings: camera_constraints(),
        flow_bounds: bounds(),
        use_authority_contract: AuthorityContractId::from("conduit.authority/use-human-media@1"),
        use_authority_grant: AuthorityGrantId::from("grant/use-opaque-track-7"),
        availability: MediaResourceAvailability::Available,
    };
    let result = MediaAcquisitionResult::Acquired(resource.clone());
    assert_eq!(acquisition, sealed_before_result);
    assert!(matches!(result, MediaAcquisitionResult::Acquired(_)));

    let requirement = MediaUseRequirement {
        kind: HumanMediaKind::Camera,
        output_port: PortId::from("frame"),
        class_id: resource.class_id.clone(),
        value_kind: resource.value_kind.clone(),
        flow_bounds: bounds(),
    };
    assert_eq!(
        select_acquired_media(&requirement, &resource, None),
        Err(MediaPlanningRefusal::UseAuthorityMissing)
    );
    let selected =
        select_acquired_media(&requirement, &resource, Some(&resource.use_authority_grant))
            .unwrap();
    assert_eq!(selected.handle_id, ResourceHandleId::from("opaque-track/7"));
    assert_eq!(selected.output_port, PortId::from("frame"));
    assert_eq!(selected.use_authority_grant, resource.use_authority_grant);
    let mut missing_port = requirement;
    missing_port.output_port = PortId::from("");
    assert_eq!(
        select_acquired_media(
            &missing_port,
            &resource,
            Some(&resource.use_authority_grant)
        ),
        Err(MediaPlanningRefusal::WrongResourceKind)
    );
}

#[test]
fn acquisition_refusals_remain_distinct() {
    assert_eq!(
        plan_media_acquisition(PlanId::from("p"), &offer(), None, request(), 0),
        Err(MediaPlanningRefusal::RequestAuthorityMissing)
    );
    assert_eq!(
        plan_media_acquisition(
            PlanId::from("p"),
            &offer(),
            Some(&authority()),
            request(),
            1
        ),
        Err(MediaPlanningRefusal::CapacityExhausted)
    );
    let outcomes = [
        MediaAcquisitionResult::Denied,
        MediaAcquisitionResult::Dismissed,
        MediaAcquisitionResult::Cancelled,
        MediaAcquisitionResult::NoMatchingDevice,
        MediaAcquisitionResult::UnsupportedConstraints,
        MediaAcquisitionResult::CapacityExhausted,
        MediaAcquisitionResult::Closed,
    ];
    for (index, left) in outcomes.iter().enumerate() {
        for right in outcomes.iter().skip(index + 1) {
            assert_ne!(left, right);
        }
    }
}

#[test]
fn later_loss_and_closure_are_not_generic_unavailability() {
    let mut resource = AcquiredMediaResource {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("browser-boot/one"),
        handle_id: ResourceHandleId::from("track"),
        class_id: ResourceClassId::from("camera"),
        value_kind: KindId::from("frame"),
        settings: camera_constraints(),
        flow_bounds: bounds(),
        use_authority_contract: AuthorityContractId::from("use"),
        use_authority_grant: AuthorityGrantId::from("use-grant"),
        availability: MediaResourceAvailability::Lost,
    };
    let requirement = MediaUseRequirement {
        kind: HumanMediaKind::Camera,
        output_port: PortId::from("frame"),
        class_id: resource.class_id.clone(),
        value_kind: resource.value_kind.clone(),
        flow_bounds: bounds(),
    };
    assert_eq!(
        select_acquired_media(&requirement, &resource, Some(&resource.use_authority_grant)),
        Err(MediaPlanningRefusal::ResourceLost)
    );
    resource.availability = MediaResourceAvailability::Closed;
    assert_eq!(
        select_acquired_media(&requirement, &resource, Some(&resource.use_authority_grant)),
        Err(MediaPlanningRefusal::ResourceClosed)
    );
}
