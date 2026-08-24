use std::collections::BTreeMap;

use conduit_core::{
    verify_plan, BootId, CapabilityId, CheckedFormId, ConnectionBase, GearId, HostId, LineId,
    OfferGeneration,
};
use conduit_form::parse_with_startup;
use conduit_planner::{
    default_placements, plan, plan_with_options, seed_planning_from_advice, PlanningAdvice,
    PlanningAdviceRefusal, PlanningOptions, SuggestedLine, SuggestedPlacement,
};
use conduit_signal::{
    pico_local_advertisement, signal_profile_catalog, triple, DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
    SIGNAL_ENCODED_LEN,
};

fn pulse_fixture() -> (
    conduit_form::CheckedForm,
    [conduit_core::HostAdvertisement; 2],
) {
    let form = parse_with_startup(
        "form advised {\n    pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("pulse Form checks");
    let first = pico_local_advertisement();
    let mut second = first.clone();
    second.host_id = HostId::from("advice/host-b");
    second.boot_id = BootId::from("advice/boot-b");
    second.offer_generation = OfferGeneration(7);
    second.capabilities[0].capability_id = CapabilityId::from("advice/pulse-b");
    (form, [first, second])
}

fn placement_advice(
    form: &conduit_form::CheckedForm,
    host: &conduit_core::HostAdvertisement,
) -> PlanningAdvice {
    PlanningAdvice {
        proposal_id: "proposal/placement-b".into(),
        request_identity: "request/model-planning-1".into(),
        run_identity: "run/deterministic-adviser-1".into(),
        checked_form_id: form.checked_form_id.clone(),
        placements: vec![SuggestedPlacement {
            gear_id: GearId::from("advised/pulse"),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: host.capabilities[0].capability_id.clone(),
        }],
        lines: vec![],
    }
}

#[test]
fn optional_advice_seeds_the_same_ordinary_planner_without_minting_plan_truth() {
    let (form, hosts) = pulse_fixture();
    let ordinary_choices = default_placements(&form, &hosts).expect("ordinary choices");
    let ordinary = plan(&form, &hosts, &ordinary_choices, &[ConnectionBase::Local])
        .expect("planning works without a model");

    let advice = placement_advice(&form, &hosts[1]);
    let seeded = seed_planning_from_advice(&form, &hosts, &[], &advice).expect("advice validates");
    assert_eq!(seeded.evidence.proposal_id, advice.proposal_id);
    assert_eq!(seeded.evidence.request_identity, advice.request_identity);
    assert_eq!(seeded.evidence.run_identity, advice.run_identity);
    assert_eq!(seeded.evidence.proposed_placements, 1);
    assert_eq!(seeded.evidence.used_placements, 1);

    let advised = plan(&form, &hosts, &seeded.placements, &[ConnectionBase::Local])
        .expect("ordinary planner validates and seals advised inputs");
    assert!(verify_plan(&ordinary) && verify_plan(&advised));
    assert_ne!(ordinary.plan_id, advised.plan_id);
    assert!(ordinary
        .fragments
        .iter()
        .all(|fragment| fragment.host_id == hosts[0].host_id));
    assert!(advised
        .fragments
        .iter()
        .all(|fragment| fragment.host_id == hosts[1].host_id));
    assert!(!format!("{advised:?}").contains(&advice.proposal_id));
}

#[test]
fn stale_or_invented_candidate_references_refuse_before_planning() {
    let (form, hosts) = pulse_fixture();
    let baseline = placement_advice(&form, &hosts[1]);

    let mut wrong_form = baseline.clone();
    wrong_form.checked_form_id = CheckedFormId::from("checked/invented");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &wrong_form),
        Err(PlanningAdviceRefusal::WrongForm)
    );

    let mut wrong_gear = baseline.clone();
    wrong_gear.placements[0].gear_id = GearId::from("advised/invented");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &wrong_gear),
        Err(PlanningAdviceRefusal::UnknownGear)
    );

    let mut wrong_host = baseline.clone();
    wrong_host.placements[0].host_id = HostId::from("advice/invented-host");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &wrong_host),
        Err(PlanningAdviceRefusal::UnknownHost)
    );

    let mut stale_boot = baseline.clone();
    stale_boot.placements[0].boot_id = BootId::from("advice/stale-boot");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &stale_boot),
        Err(PlanningAdviceRefusal::StaleBoot)
    );

    let mut stale_generation = baseline.clone();
    stale_generation.placements[0].offer_generation = OfferGeneration(6);
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &stale_generation),
        Err(PlanningAdviceRefusal::StaleOfferGeneration)
    );

    let mut wrong_capability = baseline;
    wrong_capability.placements[0].capability_id = CapabilityId::from("advice/invented-offer");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &[], &wrong_capability),
        Err(PlanningAdviceRefusal::UnknownCapability)
    );
}

#[test]
fn exact_line_advice_is_revalidated_then_sealed_only_by_ordinary_planning() {
    let exact = triple::exact_plan().expect("triple fixture");
    let form = parse_with_startup(
        include_str!("../../../fixtures/forms/triple-signal.conduit"),
        &conduit_signal::signal_startup_catalog(),
        &signal_profile_catalog(),
    )
    .expect("triple Form checks");
    let hosts = vec![
        exact.source_advertisement,
        exact.browser_advertisement,
        exact.pico_advertisement,
    ];
    let lines = vec![exact.browser_line, exact.pico_line];
    let advice = PlanningAdvice {
        proposal_id: "proposal/triple-lines".into(),
        request_identity: "request/triple-lines".into(),
        run_identity: "run/triple-lines".into(),
        checked_form_id: form.checked_form_id.clone(),
        placements: [
            ("triple-signal/pulse", 0, triple::PULSE_CAPABILITY_ID),
            ("triple-signal/local", 0, triple::STDOUT_CAPABILITY_ID),
            ("triple-signal/web", 1, triple::BROWSER_CAPABILITY_ID),
            ("triple-signal/light", 2, triple::PICO_CAPABILITY_ID),
        ]
        .into_iter()
        .map(|(gear_id, host_index, capability_id)| {
            let host = &hosts[host_index];
            SuggestedPlacement {
                gear_id: GearId::from(gear_id),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                capability_id: CapabilityId::from(capability_id),
            }
        })
        .collect(),
        lines: vec![SuggestedLine {
            source_gear_id: GearId::from("triple-signal/pulse"),
            sink_gear_id: GearId::from("triple-signal/web"),
            line_id: LineId::from(triple::BROWSER_LINE_ID),
        }],
    };
    let seeded = seed_planning_from_advice(&form, &hosts, &lines, &advice)
        .expect("exact current Line advice validates");
    let plan = plan_with_options(
        &form,
        &hosts,
        &seeded.placements,
        &[
            ConnectionBase::Local,
            ConnectionBase::WebSocket,
            ConnectionBase::UsbCdc,
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &seeded.line_candidates,
            connection_item_capacity: DISTRIBUTED_MAXIMUM_IN_FLIGHT_ITEMS,
            connection_byte_capacity: SIGNAL_ENCODED_LEN,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &lines,
        },
    )
    .expect("ordinary planner revalidates the complete proposal");
    assert!(verify_plan(&plan));
    assert_eq!(seeded.evidence.used_lines, 1);

    let mut stale = advice.clone();
    stale.lines[0].line_id = LineId::from("line/invented");
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &lines, &stale),
        Err(PlanningAdviceRefusal::UnknownLine)
    );

    let mut unavailable_lines = lines;
    unavailable_lines[0].availability.availability = conduit_core::LineAvailability::Unavailable;
    assert_eq!(
        seed_planning_from_advice(&form, &hosts, &unavailable_lines, &advice),
        Err(PlanningAdviceRefusal::LineUnavailable)
    );
}

#[test]
fn fresh_truth_changes_only_future_planning_and_never_mutates_the_active_plan() {
    let (form, hosts) = pulse_fixture();
    let advice = placement_advice(&form, &hosts[1]);
    let seeded = seed_planning_from_advice(&form, &hosts, &[], &advice).unwrap();
    let active = plan(&form, &hosts, &seeded.placements, &[ConnectionBase::Local]).unwrap();
    let sealed_active = active.clone();

    let mut fresh_hosts = hosts.clone();
    fresh_hosts[1].boot_id = BootId::from("advice/boot-b-replacement");
    fresh_hosts[1].offer_generation = OfferGeneration(1);
    assert_eq!(
        seed_planning_from_advice(&form, &fresh_hosts, &[], &advice),
        Err(PlanningAdviceRefusal::StaleBoot)
    );
    assert_eq!(active, sealed_active);
    assert!(verify_plan(&active));

    let fresh_advice = placement_advice(&form, &fresh_hosts[1]);
    let fresh = seed_planning_from_advice(&form, &fresh_hosts, &[], &fresh_advice).unwrap();
    let replacement = plan(
        &form,
        &fresh_hosts,
        &fresh.placements,
        &[ConnectionBase::Local],
    )
    .unwrap();
    assert_ne!(active.plan_id, replacement.plan_id);
    assert_eq!(active, sealed_active);
}
