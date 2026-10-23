//! Pure reference metadata source, not a fake store or a storage grant issuer.
use super::{allocation_probe, installation};
use conduit_core as c;
use conduit_environment_runtime::recording::*;
use conduit_kernel::{self as k, scheduler::*};
fn references() -> Vec<c::BoundedResourceRef> {
    (1..=2)
        .map(|version| c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: 1024,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest([version; 32]),
                expires_at: None,
            },
        })
        .collect()
}
fn step(back: &mut RecordingReferenceSource, ready: bool, fuel: u16) -> StepOutcome {
    back.step(
        &mut StepIo::test_frame([None], [false], [ready.then_some(512)], None, fuel),
        &StepInputBytes::test_frame([None], None),
    )
}
#[test]
fn reference_order_is_commit_owned_and_pressure_cancellation_and_empty_are_explicit() {
    let references = references();
    let encoded: Vec<_> = references.iter().map(|r| r.encode().unwrap()).collect();
    let mut back = RecordingReferenceSource::prepare(&references, 2).unwrap();
    let ((), memory) = allocation_probe::observe(|| {
        assert!(matches!(
            step(&mut back, true, 0),
            StepOutcome::Fail(k::Failure {
                code: k::FailureCode::InvalidLifecycle,
                ..
            })
        ));
        <RecordingReferenceSource as StepBack<1>>::step_committed(&mut back);
        assert_eq!(step(&mut back, false, 16), StepOutcome::Await);
        <RecordingReferenceSource as StepBack<1>>::step_committed(&mut back);
        assert_eq!(step(&mut back, true, 16), StepOutcome::Progress);
        assert_eq!(
            <RecordingReferenceSource as StepBack<1>>::prepared_output(&back, k::PortId(0)),
            Some(encoded[0].as_slice())
        );
        // An uncommitted retry emits the same reference, not a new generation.
        assert_eq!(step(&mut back, true, 16), StepOutcome::Progress);
        <RecordingReferenceSource as StepBack<1>>::step_committed(&mut back);
        assert_eq!(step(&mut back, true, 16), StepOutcome::Progress);
        assert_eq!(
            <RecordingReferenceSource as StepBack<1>>::prepared_output(&back, k::PortId(0)),
            Some(encoded[1].as_slice())
        );
        <RecordingReferenceSource as StepBack<1>>::step_committed(&mut back);
        assert_eq!(step(&mut back, true, 16), StepOutcome::Complete);
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let mut cancelled = RecordingReferenceSource::prepare(&references, 2).unwrap();
    assert_eq!(step(&mut cancelled, true, 16), StepOutcome::Progress);
    <RecordingReferenceSource as StepBack<1>>::cancel(&mut cancelled);
    <RecordingReferenceSource as StepBack<1>>::step_committed(&mut cancelled);
    assert!(matches!(
        step(&mut cancelled, true, 16),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::Cancelled,
            ..
        })
    ));
    let mut empty = RecordingReferenceSource::prepare(&[], 2).unwrap();
    assert_eq!(step(&mut empty, true, 16), StepOutcome::Complete);
}
#[test]
fn source_refuses_over_budget_duplicate_or_non_snapshot_references() {
    use RecordingReferenceRefusal as E;
    let mut references = references();
    assert!(matches!(
        RecordingReferenceSource::prepare(&references, 1),
        Err(E::Capacity)
    ));
    references[1] = references[0].clone();
    assert!(matches!(
        RecordingReferenceSource::prepare(&references, 2),
        Err(E::Duplicate)
    ));
    references.truncate(1);
    references[0].extent.bytes = 4097;
    assert!(matches!(
        RecordingReferenceSource::prepare(&references, 2),
        Err(E::Profile)
    ));
}
#[test]
fn actual_reference_source_is_planned_and_transferred_without_storage_authority() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_generation_source_kind(&mut startup, &mut profiles).unwrap();
    let syntax = conduit_plot::parse_syntax_document(
        "plot recorder {\n generations: environment/recording-generations\n}\n",
    );
    let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
    let source = conduit_plot::expand_canonical_plot(&checked, "recorder", &profiles).unwrap();
    let mut host = installation::host();
    let mut owner = ReadyRecordingGenerations::prepare_generations(
        &source,
        host.host_id.clone(),
        host.boot_id.clone(),
        &references(),
        2,
    )
    .unwrap();
    owner.append_to_advertisement(&mut host).unwrap();
    assert!(host.capabilities[0].host_calls.is_empty());
    assert!(host.capabilities[0].authority_requirements.is_empty());
    assert_eq!(
        generation_source_kind().outputs[0].value_kind,
        conduit_semantic_catalog::resource_snapshot_semantic_contract(false).inputs[0].value_kind
    );
    let plan = installation::plan(&source, &host);
    let fragment = &plan.fragments[0];
    let mut back = owner
        .take_selected(fragment, &fragment.placements[0].placement_id)
        .unwrap();
    assert_eq!(step(&mut back, true, 16), StepOutcome::Progress);
    assert!(matches!(
        owner.take_selected(fragment, &fragment.placements[0].placement_id),
        Err(RecoveryInstallationRefusal::AlreadySelected)
    ));
}
