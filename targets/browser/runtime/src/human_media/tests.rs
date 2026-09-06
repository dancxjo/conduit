use super::*;
use conduit_core::{
    kind_id, AuthorityContractId, BootId, BoundedResourceRef, HostId, KindId, OfferGeneration,
    ResourceClassId, ResourceExtent, ResourceHandleId, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_human::{
    HumanMediaKind, KnownPermissionState, MediaConstraints, MediaFlowBounds,
    MediaResourceAvailability,
};

fn constraints() -> MediaConstraints {
    MediaConstraints::Camera {
        minimum_width: 320,
        maximum_width: 640,
        minimum_height: 240,
        maximum_height: 480,
        maximum_frames_per_second: 30,
    }
}

fn bounds() -> MediaFlowBounds {
    MediaFlowBounds {
        maximum_value_bytes: 4096,
        maximum_queue_items: 1,
        maximum_queue_bytes: 4096,
    }
}

fn offer() -> MediaAcquisitionOffer {
    MediaAcquisitionOffer {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("boot/one"),
        offer_generation: OfferGeneration(1),
        kind: HumanMediaKind::Camera,
        operation_contract: conduit_core::HostOperationContractId::from("acquire"),
        request_authority_contract: AuthorityContractId::from("request"),
        known_permission: KnownPermissionState::Prompt,
        maximum_in_flight: 1,
        maximum_result_bytes: 1024,
    }
}

fn authority() -> MediaAcquisitionAuthority {
    MediaAcquisitionAuthority {
        grant_id: AuthorityGrantId::from("request-grant"),
        contract_id: AuthorityContractId::from("request"),
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("boot/one"),
        kind: HumanMediaKind::Camera,
    }
}

fn request() -> MediaAcquisitionRequest {
    MediaAcquisitionRequest {
        operation_id: HostOperationId::from("acquire/one"),
        constraints: constraints(),
        flow_bounds: bounds(),
    }
}

fn resource() -> AcquiredMediaResource {
    AcquiredMediaResource {
        host_id: HostId::from("browser/one"),
        boot_id: BootId::from("boot/one"),
        handle_id: ResourceHandleId::from("opaque-track/one"),
        class_id: ResourceClassId::from("camera"),
        value_kind: KindId::from("frame"),
        settings: constraints(),
        flow_bounds: bounds(),
        use_authority_contract: AuthorityContractId::from("use"),
        use_authority_grant: AuthorityGrantId::from("use-grant"),
        availability: MediaResourceAvailability::Available,
    }
}

fn playing() -> BrowserMediaSession {
    let mut session = BrowserMediaSession::new();
    session
        .seal_acquisition(
            PlanId::from("acquire-plan"),
            &offer(),
            Some(&authority()),
            request(),
        )
        .unwrap();
    let acquisition_plan = session.phase().clone();
    session.start_acquisition().unwrap();
    session
        .complete_acquisition(
            &HostOperationId::from("acquire/one"),
            128,
            MediaAcquisitionResult::Acquired(resource()),
        )
        .unwrap();
    assert!(matches!(
        acquisition_plan,
        BrowserMediaPhase::AcquisitionPlanned(_)
    ));
    let requirement = MediaUseRequirement {
        kind: HumanMediaKind::Camera,
        output_port: conduit_core::PortId::from("frame"),
        class_id: ResourceClassId::from("camera"),
        value_kind: KindId::from("frame"),
        flow_bounds: bounds(),
    };
    session
        .seal_use(
            PlanId::from("use-plan"),
            &requirement,
            Some(&AuthorityGrantId::from("use-grant")),
        )
        .unwrap();
    session.start_use().unwrap();
    session
}

fn image_content(profile: &str, bytes: u64) -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([3; 32]),
        content_profile: kind_id(profile),
        access_class: ResourceClassId::from("browser/materialized-image@1"),
        extent: ResourceExtent {
            bytes,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([4; 32]),
            expires_at: None,
        },
    }
}

#[test]
fn successful_sequence_requires_two_plans_and_observes_one_bounded_value() {
    let mut session = playing();
    assert!(matches!(
        session.phase(),
        BrowserMediaPhase::UsePlaying { plan_id, selection, .. }
            if plan_id.as_str() == "use-plan" && selection.output_port.as_str() == "frame"
    ));
    session.admit_value(4096).unwrap();
    assert_eq!(
        (session.retained_bytes(), session.observed_values()),
        (4096, 1)
    );
    assert_eq!(session.admit_value(1), Err(BrowserMediaRefusal::Pressure));
    session.release_value().unwrap();
    session.close_media().unwrap();
    assert_eq!(
        session.phase(),
        &BrowserMediaPhase::Terminal(BrowserMediaTerminal::MediaClosed)
    );
}

#[test]
fn selected_camera_materializes_the_same_portable_observation_as_other_hosts() {
    let profile = kind_id("media/image-rgba8@1");
    let mut session = playing();
    let observation = session
        .admit_image_observation(image_content(profile.as_str(), 4_096), 640, 480, &profile)
        .unwrap();
    assert_eq!(observation.width, 640);
    assert_eq!(observation.content.content_profile, profile);
    assert_eq!(
        (session.retained_bytes(), session.observed_values()),
        (4_096, 1)
    );
}

#[test]
fn image_materialization_refuses_semantic_bounds_before_consuming_flow_capacity() {
    let profile = kind_id("media/image-rgba8@1");
    let mut session = playing();
    assert_eq!(
        session.admit_image_observation(image_content(profile.as_str(), 4_096), 0, 480, &profile,),
        Err(BrowserMediaRefusal::ImageObservation(
            conduit_human::ImageObservationRefusal::InvalidDimensions
        ))
    );
    assert_eq!(
        (session.retained_bytes(), session.observed_values()),
        (0, 0)
    );
}

#[test]
fn authority_correlation_bounds_and_late_loss_are_distinct() {
    let mut absent = BrowserMediaSession::new();
    assert_eq!(
        absent.seal_acquisition(PlanId::from("p"), &offer(), None, request()),
        Err(BrowserMediaRefusal::Planning(
            MediaPlanningRefusal::RequestAuthorityMissing
        ))
    );
    let mut session = BrowserMediaSession::new();
    session
        .seal_acquisition(PlanId::from("p"), &offer(), Some(&authority()), request())
        .unwrap();
    session.start_acquisition().unwrap();
    assert_eq!(
        session.complete_acquisition(
            &HostOperationId::from("wrong"),
            64,
            MediaAcquisitionResult::Denied
        ),
        Err(BrowserMediaRefusal::CompletionOperationMismatch)
    );
    let mut use_session = playing();
    assert_eq!(
        use_session.admit_value(4097),
        Err(BrowserMediaRefusal::ValueTooLarge)
    );
    use_session.device_lost().unwrap();
    assert_eq!(
        use_session.phase(),
        &BrowserMediaPhase::Terminal(BrowserMediaTerminal::DeviceLost)
    );
}

#[test]
fn every_acquisition_terminal_remains_distinct() {
    let results = [
        MediaAcquisitionResult::Denied,
        MediaAcquisitionResult::Dismissed,
        MediaAcquisitionResult::Cancelled,
        MediaAcquisitionResult::NoMatchingDevice,
        MediaAcquisitionResult::UnsupportedConstraints,
        MediaAcquisitionResult::CapacityExhausted,
        MediaAcquisitionResult::Closed,
    ];
    let mut terminals = Vec::new();
    for result in results {
        let mut session = BrowserMediaSession::new();
        session
            .seal_acquisition(PlanId::from("p"), &offer(), Some(&authority()), request())
            .unwrap();
        session.start_acquisition().unwrap();
        session
            .complete_acquisition(&HostOperationId::from("acquire/one"), 32, result)
            .unwrap();
        terminals.push(session.phase().clone());
    }
    terminals.sort_by_key(|value| format!("{value:?}"));
    terminals.dedup();
    assert_eq!(terminals.len(), 7);
}
