//! Actual prepared recovery software; Host/Boot and historical provenance are
//! explicit CPU fixtures. No browser storage, membership or Body start is claimed.
use super::{allocation_probe, history};
use conduit_core as c;
use conduit_environment_runtime::{observation::ObservationContract, recording::*};
fn source() -> conduit_plot::ExpandedCanonicalPlot {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_recovery_kind(&mut startup, &mut profiles).unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot recorder {\n recovery: environment/recover-recording\n}\n",
    );
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    conduit_plot::expand_canonical_plot(&checked, "recorder", &profiles).unwrap()
}
fn host() -> c::HostAdvertisement {
    c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/recorder".into(),
        boot_id: "fixture/recorder/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/recorder".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn ready(source: &conduit_plot::ExpandedCanonicalPlot) -> ReadyRecordingRecovery {
    let host = host();
    ReadyRecordingRecovery::prepare(
        source,
        host.host_id,
        host.boot_id,
        history::context(),
        &ObservationContract::prepare().unwrap(),
        2,
    )
    .unwrap()
}
fn plan(source: &conduit_plot::ExpandedCanonicalPlot, host: &c::HostAdvertisement) -> c::Plan {
    let hosts = core::slice::from_ref(host);
    let placements = conduit_planner::default_expanded_placements(source, hosts).unwrap();
    conduit_planner::plan_expanded_canonical_with_options(
        source,
        hosts,
        &placements,
        &[],
        conduit_planner::PlanningOptions {
            connection_bases: &std::collections::BTreeMap::new(),
            line_candidates: &std::collections::BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
}
#[test]
fn initialized_recovery_is_selected_once_and_historical_context_is_independent() {
    use conduit_kernel::{scheduler::*, ValueRef};
    let source = source();
    let mut owner = ready(&source);
    let mut host = host();
    owner.append_to_advertisement(&mut host).unwrap();
    let plan = plan(&source, &host);
    let fragment = &plan.fragments[0];
    let placement = &fragment.placements[0].placement_id;
    let mut back = owner.take_selected(fragment, placement).unwrap();
    assert!(matches!(
        owner.take_selected(fragment, placement),
        Err(RecoveryInstallationRefusal::AlreadySelected)
    ));
    let mut fresh = host.clone();
    fresh.capabilities.clear();
    owner.append_to_advertisement(&mut fresh).unwrap();
    assert!(fresh.capabilities.is_empty());
    let content = history::chunk(&[(1, 10)]);
    let ((), memory) = allocation_probe::observe(|| {
        let value = ValueRef {
            slot: 0,
            generation: 1,
            byte_len: content.len() as u32,
        };
        let outcome = back.step(
            &mut StepIo::test_frame([Some(value)], [false], [None], None, 16),
            &StepInputBytes::test_frame([Some(content.as_bytes())], None),
        );
        assert_eq!(outcome, StepOutcome::Progress);
        assert_eq!(back.recovered(), RecoveredHistory::default());
        <RecordingRecoveryBack as StepBack<1>>::step_committed(&mut back);
        assert_eq!(back.recovered().samples, 1);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}
#[test]
fn wrong_advertisement_or_resealed_selection_cannot_consume_the_retained_back() {
    use RecoveryInstallationRefusal as E;
    let source = source();
    let mut owner = ready(&source);
    let mut actual = host();
    for mode in 0..2 {
        let mut invalid = actual.clone();
        if mode == 0 {
            invalid.host_id = "foreign".into();
        } else {
            invalid.boot_id = "stale".into();
        }
        let before = invalid.clone();
        assert_eq!(
            owner.append_to_advertisement(&mut invalid),
            Err(E::Advertisement)
        );
        assert_eq!(invalid, before);
    }
    owner.append_to_advertisement(&mut actual).unwrap();
    let before = actual.clone();
    assert_eq!(
        owner.append_to_advertisement(&mut actual),
        Err(E::Advertisement)
    );
    assert_eq!(actual, before);
    let plan = plan(&source, &actual);
    let placement = plan.fragments[0].placements[0].placement_id.clone();
    for mode in 0..5 {
        let mut fragments = plan.fragments.clone();
        let gear = &mut fragments[0].placements[0];
        match mode {
            0 => gear.artifact_id = "foreign/artifact".into(),
            1 => gear.offer_generation = c::OfferGeneration(2),
            2 => gear.limits.max_queue_bytes += 1,
            3 => gear.configuration.push(c::ConfigurationEntry {
                key: "extra".into(),
                value: c::ConfigurationValue::U64(1),
            }),
            _ => gear.implementation_id = "foreign/implementation".into(),
        }
        let forged = c::seal_plan_with_realization_backs_and_completion(
            c::PlotIdentity {
                source_document_id: plan.source_document_id.clone(),
                checked_plot_id: plan.checked_plot_id.clone(),
                expanded_plot_id: plan.expanded_plot_id.clone(),
            },
            plan.completion_policy,
            plan.realization_backs.clone(),
            fragments,
        );
        assert!(matches!(
            owner.take_selected(&forged.fragments[0], &placement),
            Err(E::Selection)
        ));
    }
    assert!(owner.take_selected(&plan.fragments[0], &placement).is_ok());
}

#[test]
fn recovery_owner_refuses_ambiguous_or_changed_checked_meaning() {
    for mode in 0..3 {
        let mut source = source();
        match mode {
            0 => source.gears.push(source.gears[0].clone()),
            1 => source.gears[0].inputs[0].value_kind = "foreign/value".into(),
            _ => source.gears[0].configuration.push(c::ConfigurationEntry {
                key: "extra".into(),
                value: c::ConfigurationValue::U64(1),
            }),
        }
        let host = host();
        assert!(matches!(
            ReadyRecordingRecovery::prepare(
                &source,
                host.host_id,
                host.boot_id,
                history::context(),
                &ObservationContract::prepare().unwrap(),
                2
            ),
            Err(RecoveryInstallationRefusal::Source)
        ));
    }
}
