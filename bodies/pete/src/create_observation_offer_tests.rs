use super::*;
use crate::{decode_observation_bundle, lower_charging_sources, lower_group_zero};

const CONTACT_FORM: &str = "form contact_sample {\n contact: robotics/observe-contact\n}\n";

fn evidence() -> CreateObservationEvidence {
    CreateObservationEvidence {
        host_id: HostId::from("host/std-create"),
        boot_id: BootId::from("boot/1"),
        offer_generation: OfferGeneration(7),
        serial_base_id: "base/tty-create".into(),
        robot_identity: "create/serial-1".into(),
        session_resource_id: "session/create-1".into(),
        mode: OiMode::Safe,
        observed_at_tick: 90,
        maximum_age_ticks: 20,
    }
}

fn observation() -> CreatePortableObservation {
    let mut group = [0_u8; 26];
    group[0] = 0b11;
    group[6] = 1;
    group[10] = 137;
    group[11] = 5;
    group[16] = 3;
    group[17..19].copy_from_slice(&14_000_u16.to_be_bytes());
    group[19..21].copy_from_slice(&100_i16.to_be_bytes());
    group[22..24].copy_from_slice(&1_000_u16.to_be_bytes());
    group[24..26].copy_from_slice(&2_000_u16.to_be_bytes());
    let mut frame = vec![19, 29, 0];
    frame.extend_from_slice(&group);
    frame.extend_from_slice(&[34, 2]);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    let bundle = decode_observation_bundle(&frame).unwrap();
    CreatePortableObservation {
        group_zero: lower_group_zero(&bundle.group_zero).unwrap(),
        charging_sources: lower_charging_sources(&bundle.charging_sources).unwrap(),
    }
}

fn snapshot() -> CreateObservationSnapshot {
    CreateObservationSnapshot {
        host_id: HostId::from("host/std-create"),
        boot_id: BootId::from("boot/1"),
        offer_generation: OfferGeneration(7),
        serial_base_id: "base/tty-create".into(),
        robot_identity: "create/serial-1".into(),
        observation_generation: 4,
        observed_at_tick: 90,
        maximum_age_ticks: 20,
        observation: observation(),
        odometry: Some(CreateOdometrySample {
            value: conduit_robotics::OdometryObservation::new(-120, 0, 523_599).unwrap(),
            frame_generation: 1,
            sample_generation: 4,
        }),
    }
}

#[test]
fn live_advertisement_is_exact_bounded_and_shares_one_session() {
    let advertisement = live_create_observation_advertisement(&evidence(), 100).unwrap();
    assert_eq!(advertisement.capabilities.len(), MAXIMUM_CHANNELS);
    assert_eq!(advertisement.resources.len(), 3);
    assert!(advertisement
        .resources
        .iter()
        .all(|resource| resource.capacity_units == 1));
    for capability in &advertisement.capabilities {
        assert_eq!(capability.resource_requirements.len(), 3);
        assert_eq!(capability.host_operations.len(), 1);
        assert_eq!(capability.host_operations[0].maximum_in_flight, 1);
        assert_eq!(capability.host_operations[0].maximum_input_bytes, 0);
        assert_eq!(capability.limits.max_active_instances, 1);
        assert_eq!(capability.limits.max_queue_items, 1);
    }
    assert_ne!(
        CreateObservationChannel::VirtualWall.implementation_id(),
        CreateObservationChannel::Infrared.implementation_id()
    );
}

#[test]
fn missing_stale_and_unsupported_evidence_refuse_before_offer() {
    let mut value = evidence();
    value.robot_identity.clear();
    assert_eq!(
        live_create_observation_advertisement(&value, 100),
        Err(CreateObservationOfferRefusal::MissingIdentity)
    );
    let mut value = evidence();
    value.mode = OiMode::Passive;
    assert_eq!(
        live_create_observation_advertisement(&value, 100),
        Err(CreateObservationOfferRefusal::UnsupportedMode)
    );
    assert_eq!(
        live_create_observation_advertisement(&evidence(), 111),
        Err(CreateObservationOfferRefusal::StaleEvidence)
    );
}

#[test]
fn one_correlated_observation_encodes_each_portable_channel_exactly() {
    let snapshot = snapshot();
    for channel in CreateObservationChannel::ALL {
        let encoded = encode_create_observation(&snapshot, channel, 100)
            .unwrap()
            .expect("fixture has every optional channel");
        assert_eq!(
            encoded.as_bytes().len() as u32,
            observation_offer(channel).host_operations[0].maximum_output_bytes
        );
    }
    assert_ne!(
        encode_create_observation(&snapshot, CreateObservationChannel::VirtualWall, 100)
            .unwrap()
            .unwrap(),
        encode_create_observation(&snapshot, CreateObservationChannel::Infrared, 100)
            .unwrap()
            .unwrap()
    );
    let bump = encode_create_observation(&snapshot, CreateObservationChannel::BumpAggregate, 100)
        .unwrap()
        .unwrap();
    assert!(conduit_core::InfoBool::decode(bump.as_bytes())
        .unwrap()
        .get());
    let mut clear = snapshot.clone();
    clear.observation.group_zero.contact = conduit_robotics::ContactObservation::new(0).unwrap();
    let clear_bump =
        encode_create_observation(&clear, CreateObservationChannel::BumpAggregate, 100)
            .unwrap()
            .unwrap();
    assert!(!conduit_core::InfoBool::decode(clear_bump.as_bytes())
        .unwrap()
        .get());
    assert_eq!(
        encode_create_observation(&snapshot, CreateObservationChannel::Contact, 111),
        Err(CreateObservationEncodeRefusal::StaleObservation)
    );
}

#[test]
fn mechanism_free_form_plans_to_exact_create_realization() {
    for forbidden in ["create", "uart", "serial", "opcode", "pete"] {
        assert!(!CONTACT_FORM.contains(forbidden));
    }
    let (startup, profile) = crate::catalogs().unwrap();
    let syntax = conduit_form::parse_syntax_document(CONTACT_FORM);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded =
        conduit_form::expand_canonical_form(&checked, "contact_sample", &profile).unwrap();
    let host = live_create_observation_advertisement(&evidence(), 100).unwrap();
    let placements =
        conduit_planner::default_expanded_placements(&expanded, std::slice::from_ref(&host))
            .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded,
        &[host],
        &placements,
        &[conduit_core::BaseImplementationId::from(
            "conduit.base/local@1",
        )],
    )
    .unwrap();
    let placement = &plan.fragments[0].placements[0];
    assert_eq!(
        placement.implementation_id.as_str(),
        CreateObservationChannel::Contact.implementation_id()
    );
    assert_eq!(placement.host_operations.len(), 1);
    assert_eq!(placement.resources.len(), 3);
}

#[test]
fn one_form_swaps_tiny_hosts_and_encodes_the_selected_fragment() {
    use conduit_core::{assigned_plan_payload_digest, AssignedPlanMaxima};
    use conduit_embedded_build::{
        encode_assigned_plan, generate_embedded_plan, EmbeddedImageBounds,
    };
    use conduit_plan_lowering::lowering::lower_plan_fragment;

    let plan_for = |host_id: &str, boot_id: &str| {
        let (startup, profile) = crate::catalogs().unwrap();
        let syntax = conduit_form::parse_syntax_document(CONTACT_FORM);
        let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
        let expanded =
            conduit_form::expand_canonical_form(&checked, "contact_sample", &profile).unwrap();
        let mut target_evidence = evidence();
        target_evidence.host_id = HostId::from(host_id);
        target_evidence.boot_id = BootId::from(boot_id);
        let host = live_create_observation_advertisement(&target_evidence, 100).unwrap();
        let placements =
            conduit_planner::default_expanded_placements(&expanded, std::slice::from_ref(&host))
                .unwrap();
        conduit_planner::plan_expanded_canonical(
            &expanded,
            &[host],
            &placements,
            &[conduit_core::BaseImplementationId::from(
                "conduit.base/local@1",
            )],
        )
        .unwrap()
    };

    let pico = plan_for("host/pico-create", "boot/pico-1");
    let avr = plan_for("host/avr-create", "boot/avr-1");
    let pico_placement = &pico.fragments[0].placements[0];
    let avr_placement = &avr.fragments[0].placements[0];

    assert_eq!(pico.source_document_id, avr.source_document_id);
    assert_eq!(pico.checked_form_id, avr.checked_form_id);
    assert_eq!(pico.expanded_form_id, avr.expanded_form_id);
    assert_eq!(pico_placement.kind_id, avr_placement.kind_id);
    assert_eq!(
        pico_placement.implementation_id,
        avr_placement.implementation_id
    );
    assert_eq!(pico_placement.artifact_id, avr_placement.artifact_id);
    assert_eq!(
        pico_placement.host_operations,
        avr_placement.host_operations
    );
    assert_eq!(
        pico_placement.resources.len(),
        avr_placement.resources.len()
    );
    assert_ne!(pico_placement.host_id, avr_placement.host_id);
    assert_ne!(pico_placement.boot_id, avr_placement.boot_id);

    let fragment = &avr.fragments[0];
    let lowered = lower_plan_fragment(fragment).unwrap();
    let generated =
        generate_embedded_plan(fragment, &lowered, EmbeddedImageBounds::HOST_TOOLING).unwrap();
    let assigned = encode_assigned_plan(&generated, AssignedPlanMaxima::TINY_HOST).unwrap();

    assert_eq!(generated.plan_id, avr.plan_id.as_str());
    assert_eq!(generated.fragment_id, fragment.fragment_id.as_str());
    assert_eq!(generated.host_id, "host/avr-create");
    assert_eq!(generated.boot_id, "boot/avr-1");
    assert!(!assigned_plan_payload_digest(&assigned)
        .iter()
        .all(|byte| *byte == 0));
    assert!(assigned.len() <= usize::from(AssignedPlanMaxima::TINY_HOST.encoded_bytes));
}
