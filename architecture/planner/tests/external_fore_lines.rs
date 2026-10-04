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

fn paired_fore_plan() -> Plan {
    let first = fore_plan().fragments.remove(0);
    let mut second = common::fragment();
    second.host_id = HostId::from("peer");
    second.boot_id = BootId::from("peer-boot");
    second.placements[0].host_id = second.host_id.clone();
    second.placements[0].boot_id = second.boot_id.clone();
    second.placements[0].gear_id = GearId::from("peer-cell");
    second.placements[0].placement_id = PlacementId::from("peer-placement");
    second.placements[0].limits.max_queue_bytes = 2;
    second.startup_order = vec![second.placements[0].placement_id.clone()];
    second.states[0].state_id = StateId::from("peer-state");
    second.states[0].gear_id = second.placements[0].gear_id.clone();
    second.fore_ports.push(PlannedForePort {
        front_port_id: port_id("in"),
        direction: PortDirection::Input,
        placement_id: second.placements[0].placement_id.clone(),
        gear_port_id: port_id("next"),
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
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: first.source_document_id.clone(),
            checked_plot_id: first.checked_plot_id.clone(),
            expanded_plot_id: first.expanded_plot_id.clone(),
        },
        vec![first, second],
    );
    assert!(verify_plan(&plan));
    plan
}

#[test]
fn two_host_fragments_account_for_one_lines_total_capacity() {
    let plan = paired_fore_plan();
    let mut peer_choice = choice();
    peer_choice.host_id = HostId::from("peer");
    peer_choice.boot_id = BootId::from("peer-boot");
    peer_choice.direction = PortDirection::Input;
    peer_choice.front_port_id = port_id("in");
    peer_choice.peer_host_id = HostId::from("host");
    peer_choice.peer_boot_id = BootId::from("boot");
    let mut broad = offer();
    broad.binding.limits.maximum_in_flight_items = 2;
    broad.binding.limits.maximum_buffered_bytes = 4;
    let admitted = bind(&plan, &[choice(), peer_choice.clone()], &[broad]).unwrap();
    assert!(verify_plan(&admitted));

    let narrow = offer();
    assert!(matches!(
        bind(
            &plan,
            &[choice(), peer_choice],
            std::slice::from_ref(&narrow)
        ),
        Err(PlannerError::InvalidLineOffer(_))
    ));
    let mut overbooked = admitted;
    for fragment in &mut overbooked.fragments {
        fragment.fore_ports[0].selected_line = Some(narrow.admitted_line());
    }
    let overbooked = seal_plan(
        PlotIdentity {
            source_document_id: overbooked.source_document_id,
            checked_plot_id: overbooked.checked_plot_id,
            expanded_plot_id: overbooked.expanded_plot_id,
        },
        overbooked.fragments,
    );
    assert!(overbooked.fragments.iter().all(verify_plan_fragment));
    assert!(!verify_plan(&overbooked));
}
