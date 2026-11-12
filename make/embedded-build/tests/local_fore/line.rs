//! Checked Source -> sealed selected Line -> schema8 -> real kernel tables.
//! Pure preparation proof; no carrier, membership or execution is claimed.
use super::*;
fn offer() -> c::LineOffer {
    let line_id = c::LineId::from("fixture/line");
    let binding_id = c::LinkBindingId::from("fixture/binding");
    c::LineOffer {
        line_id: line_id.clone(),
        availability: c::LineAvailabilitySign {
            line_id,
            binding_id: binding_id.clone(),
            availability: c::LineAvailability::Ready,
            sign_id: "fixture/ready".into(),
        },
        binding: c::LinkBinding {
            binding_id,
            source: c::LinkEndpoint {
                host_id: "fixture/host".into(),
                boot_id: "fixture/boot".into(),
                endpoint_id: "fixture/source".into(),
            },
            sink: c::LinkEndpoint {
                host_id: "fixture/peer".into(),
                boot_id: "fixture/peer-boot".into(),
                endpoint_id: "fixture/sink".into(),
            },
            base: "conduit.base/usb-cdc-acm@1".into(),
            base_instance_id: "fixture/serial".into(),
            credential: c::LinkCredentialReference::None,
            authority: c::LinkAuthorityReference::ProcessOwned,
            limits: c::LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 64,
                maximum_buffered_bytes: 64,
                maximum_frame_bytes: 4096,
            },
        },
        contract: c::LineContract {
            scope: c::LineScope::PointToPoint,
            traffic_shape: c::LineTrafficShape::Message,
            duplex: c::LineDuplex::FullDuplex,
            ordering: c::LineOrdering::Ordered,
            reliability: c::LineReliability::Reliable,
            continuation: c::LineContinuation::None,
            security: c::LineSecurity::PhysicalPossession,
        },
    }
}
fn selected_plan() -> c::Plan {
    let original = plan(64);
    let choice = conduit_planner::ExternalForeLineChoice {
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        direction: c::PortDirection::Output,
        front_port_id: "result".into(),
        track: c::ConnectionTrack::Payload,
        peer_host_id: "fixture/peer".into(),
        peer_boot_id: "fixture/peer-boot".into(),
        line_id: "fixture/line".into(),
    };
    conduit_planner::bind_external_fore_lines(
        &original,
        &[choice],
        &[offer()],
        &["conduit.base/usb-cdc-acm@1".into()],
    )
    .unwrap()
}
fn session() -> b::SelectedForeSession {
    b::SelectedForeSession {
        front_port_id: "result".into(),
        direction: c::PortDirection::Output,
        track: c::ConnectionTrack::Payload,
        session: c::AssignedIdentity::from_text("fixture/session"),
        session_contract: c::AssignedIdentity::from_text("fixture/session-contract"),
    }
}
fn project(
    plan: &c::Plan,
    sessions: &[b::SelectedForeSession],
) -> Result<b::GeneratedLineForePlan, b::GenerationError> {
    b::generate_line_fore_native_plan(
        plan,
        &plan.fragments[0].fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536,
        },
        2,
        sessions,
    )
}
fn maxima() -> c::AssignedPlanMaxima {
    c::AssignedPlanMaxima {
        encoded_bytes: 16384,
        runtime_state_bytes: 32768,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    }
}
#[test]
fn original_checked_source_selected_line_reaches_exact_kernel_tables() {
    let plan = selected_plan();
    let original_id = plan.plan_id.clone();
    let generated = project(&plan, &[session()]).unwrap();
    assert_eq!(plan.plan_id, original_id);
    assert!(c::verify_plan(&plan));
    assert_eq!(generated.profile().local.len(), 1);
    assert_eq!(generated.profile().lines.len(), 1);
    let bytes = generated.encode(maxima()).unwrap();
    let computations = b::assigned_computation_requirements(generated.kernel()).unwrap();
    let profile = c::AssignedComputationProfile {
        maximum_text_bytes: 65535,
        maximum_total_text_bytes: u32::MAX,
        requirements: &computations,
    };
    let required = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text("fixture/host"),
        boot: c::AssignedIdentity::from_text("fixture/boot"),
        host_calls: &[],
        resources: &[],
        remote_bindings: generated.remote_bindings(),
    };
    let identity = c::decode_assigned_plan_with_line_fore(
        &bytes,
        maxima(),
        required,
        profile,
        generated.profile(),
    )
    .unwrap();
    assert_eq!(
        identity.plan,
        c::AssignedIdentity::from_text(plan.plan_id.as_str())
    );
    let pins = generated.layout_requirements(maxima()).unwrap();
    let layout = conduit_kernel::assigned::ResidentLayout {
        records: &pins,
        counts: identity.counts,
    };
    let tables =
        conduit_kernel::assigned::AssignedKernelTables::<1, 2, 1, 1, 1, 0>::prepare_with_line_fore(
            &bytes,
            maxima(),
            required,
            profile,
            &layout,
            generated.profile(),
        )
        .unwrap();
    assert_eq!(tables.identity, identity);
    assert_eq!(tables.cords.len(), 2);
    assert!(c::decode_assigned_plan_with_fore(
        &bytes,
        maxima(),
        required,
        profile,
        generated.profile().local
    )
    .is_err());
    assert!(
        conduit_kernel::assigned::AssignedKernelTables::<1, 2, 1, 1, 1, 0>::prepare_with_fore(
            &bytes,
            maxima(),
            required,
            profile,
            &layout,
            generated.profile().local
        )
        .is_err()
    );
    assert!(b::generate_fore_native_plan(
        &plan,
        &plan.fragments[0].fragment_id,
        b::EmbeddedImageBounds::HOST_TOOLING,
        b::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 65536
        },
        2
    )
    .is_err());
    let mut small = maxima();
    small.encoded_bytes = (bytes.len() - 1) as u16;
    assert!(generated.encode(small).is_err());
    small = maxima();
    small.counts[9] = 1;
    assert!(generated.encode(small).is_err());
}
#[test]
fn session_choices_must_exactly_cover_the_original_selected_fore() {
    let plan = selected_plan();
    assert!(project(&plan, &[]).is_err());
    assert!(project(&plan, &[session(), session()]).is_err());
    let mut wrong = session();
    wrong.front_port_id = "foreign".into();
    assert!(project(&plan, &[wrong]).is_err());
    let mut wrong = session();
    wrong.direction = c::PortDirection::Input;
    assert!(project(&plan, &[wrong]).is_err());
    assert!(project(&super::plan(64), &[session()]).is_err());
    let mut stale = plan.clone();
    stale.fragments[0]
        .fore_ports
        .iter_mut()
        .find_map(|p| p.selected_line.as_mut())
        .unwrap()
        .binding
        .sink
        .boot_id = "stale".into();
    assert!(!c::verify_plan(&stale));
    assert!(project(&stale, &[session()]).is_err());
}
#[test]
fn full_selected_line_commitment_covers_every_immutable_realization_fact() {
    let line = offer().admitted_line();
    let expected = c::admitted_line_commitment(&line);
    assert_eq!(expected, c::admitted_line_commitment(&line));
    macro_rules! field {
        ($($path:ident).+, $value:expr) => {{
            let mut next = line.clone();
            next.$($path).+ = $value;
            assert_ne!(expected, c::admitted_line_commitment(&next));
        }};
    }
    field!(line_id, "other".into());
    field!(binding.binding_id, "other".into());
    field!(binding.source.host_id, "other".into());
    field!(binding.source.boot_id, "other".into());
    field!(binding.source.endpoint_id, "other".into());
    field!(binding.sink.host_id, "other".into());
    field!(binding.sink.boot_id, "other".into());
    field!(binding.sink.endpoint_id, "other".into());
    field!(binding.base, "other".into());
    field!(binding.base_instance_id, "other".into());
    field!(
        binding.credential,
        c::LinkCredentialReference::Opaque("other".into())
    );
    field!(
        binding.authority,
        c::LinkAuthorityReference::Grant("other".into())
    );
    field!(binding.limits.maximum_in_flight_items, 2);
    field!(binding.limits.maximum_payload_bytes, 65);
    field!(binding.limits.maximum_buffered_bytes, 65);
    field!(binding.limits.maximum_frame_bytes, 4097);
    field!(contract.scope, c::LineScope::Machine);
    field!(contract.traffic_shape, c::LineTrafficShape::ByteStream);
    field!(contract.duplex, c::LineDuplex::Simplex);
    field!(contract.ordering, c::LineOrdering::Unordered);
    field!(contract.reliability, c::LineReliability::BestEffort);
    field!(
        contract.continuation,
        c::LineContinuation::BoundedSessionReconciliation
    );
    field!(contract.security, c::LineSecurity::AuthenticatedEncrypted);
}
