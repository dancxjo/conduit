use conduit_core::*;
use conduit_plan_lowering::lowering::{
    lower_plan_fragment_for_profile, FIXED_KERNEL_STORAGE_PROFILE,
};
use conduit_planner::{bind_external_fore_lines, ExternalForeLineChoice, PlannerError};

#[path = "../../core/tests/common/sealed_state.rs"]
mod common;

fn fore_plan() -> Plan {
    let mut fragment = common::fragment();
    fragment.placements[0].limits.max_queue_bytes = 2;
    fragment.fore_ports.push(PlannedForePort {
        front_port_id: port_id("out"),
        direction: PortDirection::Output,
        placement_id: PlacementId::from("placement"),
        gear_port_id: port_id("current"),
        value_kind: kind_id("fixture/byte@1"),
        value_contract: None,
        abnormal_kind: None,
        track: ConnectionTrack::Payload,
        temporal: PortTemporal::Value,
        pressure_policy: DeliveryPressurePolicy::PreserveOrder,
        item_capacity: 1,
        byte_capacity: 2,
        selected_line: None,
    });
    let plan = common::seal(fragment);
    assert!(verify_plan(&plan));
    plan
}

fn offer() -> LineOffer {
    let line_id = LineId::from("test/line");
    let binding_id = LinkBindingId::from("test/binding");
    LineOffer {
        line_id: line_id.clone(),
        availability: LineAvailabilitySign {
            line_id,
            binding_id: binding_id.clone(),
            availability: LineAvailability::Ready,
            sign_id: SignId::from("test/ready"),
        },
        binding: LinkBinding {
            binding_id,
            source: LinkEndpoint {
                host_id: HostId::from("host"),
                boot_id: BootId::from("boot"),
                endpoint_id: LinkEndpointId::from("host/egress"),
            },
            sink: LinkEndpoint {
                host_id: HostId::from("peer"),
                boot_id: BootId::from("peer-boot"),
                endpoint_id: LinkEndpointId::from("peer/ingress"),
            },
            base: BaseImplementationId::from("test/remote-base"),
            base_instance_id: BaseInstanceId::from("test/remote-base-instance"),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 2,
                maximum_buffered_bytes: 2,
                maximum_frame_bytes: 2,
            },
        },
        contract: LineContract {
            scope: LineScope::Machine,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::ProcessBoundary,
        },
    }
}

fn choice() -> ExternalForeLineChoice {
    ExternalForeLineChoice {
        host_id: HostId::from("host"),
        boot_id: BootId::from("boot"),
        direction: PortDirection::Output,
        front_port_id: port_id("out"),
        track: ConnectionTrack::Payload,
        peer_host_id: HostId::from("peer"),
        peer_boot_id: BootId::from("peer-boot"),
        line_id: LineId::from("test/line"),
    }
}

fn bind(
    plan: &Plan,
    choices: &[ExternalForeLineChoice],
    offers: &[LineOffer],
) -> Result<Plan, PlannerError> {
    bind_external_fore_lines(
        plan,
        choices,
        offers,
        &[BaseImplementationId::from("test/remote-base")],
    )
}

#[test]
fn selected_line_changes_only_bound_identity_and_survives_lowering() {
    let original = fore_plan();
    assert_eq!(
        bind(&original, &[], &[offer()]).unwrap().plan_id,
        original.plan_id
    );
    let bound = bind(&original, &[choice()], &[offer()]).unwrap();
    assert_ne!(bound.plan_id, original.plan_id);
    assert_eq!(original.fragments[0].fore_ports[0].selected_line, None);
    assert_eq!(
        bound.fragments[0].fore_ports[0].selected_line,
        Some(offer().admitted_line())
    );
    assert!(verify_plan(&bound));
    let profile = FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(1, 1)
        .unwrap();
    let lowered = lower_plan_fragment_for_profile(&bound.fragments[0], profile).unwrap();
    assert_eq!(
        lowered.fore_ports[0].selected_line,
        Some(offer().admitted_line())
    );
    let mut later_sign = offer();
    later_sign.availability.availability = LineAvailability::Unavailable;
    assert_eq!(later_sign.admitted_line(), offer().admitted_line());
    assert!(verify_plan(&bound));

    let mut changed = bound.clone();
    changed.fragments[0].fore_ports[0]
        .selected_line
        .as_mut()
        .unwrap()
        .binding
        .source
        .endpoint_id = LinkEndpointId::from("changed");
    assert!(!verify_plan(&changed));
}

#[test]
fn missing_wrong_direction_stale_and_unavailable_offers_refuse() {
    let plan = fore_plan();
    assert!(matches!(
        bind(&plan, &[choice()], &[]),
        Err(PlannerError::LineOfferMissing(_))
    ));
    let mut wrong = offer();
    core::mem::swap(&mut wrong.binding.source, &mut wrong.binding.sink);
    assert!(matches!(
        bind(&plan, &[choice()], &[wrong]),
        Err(PlannerError::LineOfferMissing(_))
    ));
    let mut stale = offer();
    stale.binding.source.boot_id = BootId::from("stale-boot");
    assert!(matches!(
        bind(&plan, &[choice()], &[stale]),
        Err(PlannerError::LineOfferMissing(_))
    ));
    let mut unavailable = offer();
    unavailable.availability.availability = LineAvailability::Unavailable;
    assert!(matches!(
        bind(&plan, &[choice()], &[unavailable]),
        Err(PlannerError::LineOfferUnavailable(_))
    ));
    assert!(matches!(
        bind(&plan, &[choice(), choice()], &[offer()]),
        Err(PlannerError::InvalidLineOffer(_))
    ));
}

#[test]
fn underbounded_or_changed_sign_identity_refuses_without_truncation() {
    let plan = fore_plan();
    let mut small = offer();
    small.binding.limits.maximum_payload_bytes = 1;
    assert!(matches!(
        bind(&plan, &[choice()], &[small]),
        Err(PlannerError::LineOfferUnavailable(_))
    ));
    let mut wrong_sign = offer();
    wrong_sign.availability.binding_id = LinkBindingId::from("other");
    assert!(matches!(
        bind(&plan, &[choice()], &[wrong_sign]),
        Err(PlannerError::InvalidLineOffer(_))
    ));
}

#[test]
fn shared_line_cannot_overbook_two_external_fores() {
    let mut fragment = fore_plan().fragments.remove(0);
    let mut second = fragment.fore_ports[0].clone();
    second.front_port_id = port_id("out2");
    fragment.fore_ports.push(second);
    let plan = common::seal(fragment);
    assert!(verify_plan(&plan));
    let mut second_choice = choice();
    second_choice.front_port_id = port_id("out2");
    assert!(matches!(
        bind(&plan, &[choice(), second_choice], &[offer()]),
        Err(PlannerError::InvalidLineOffer(_))
    ));
}
