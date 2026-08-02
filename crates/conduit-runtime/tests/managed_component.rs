use conduit_core::{
    ArtifactDigest, CompatibilityOutcome, DescriptorRef, ExecutorKind,
    ExplicitSatisfactionRequirement, Id, PinnedDescriptor, SatisfactionFacet, SatisfactionMethod,
    SatisfactionObligation, SatisfactionPin, SatisfactionProof, SatisfactionReason,
    SatisfactionRole, SemanticHash, validate_satisfaction_proof,
};
use conduit_runtime::{
    CompiledInHostService, HTTP_LISTENER_CONTRACT, Handler, InstalledArtifactRegistration,
    InstalledImplementationRegistration, MANAGED_COMPONENT_SCHEMA_VERSION, ManagedAdapterBoundary,
    ManagedArtifactIdentity, ManagedCleanupState, ManagedComponentDescriptor,
    ManagedComponentIdentity, ManagedComponentMachine, ManagedGrantState, ManagedLeaseState,
    ManagedLifecycleAction, ManagedLifecycleAuthority, ManagedLifecycleFacets,
    ManagedLifecycleProgress, ManagedLifecycleReason, ManagedLifecycleRequest,
    ManagedLifecycleState, ManagedProviderAvailability, ManagedProviderEvent, ManagedResourceState,
    ManagedRuntimeReadiness, Registry, ResolutionError,
};
use sha2::Digest as _;

struct NoopManagedHandler;

impl Handler for NoopManagedHandler {}

fn noop_factory() -> Box<dyn Handler> {
    Box::new(NoopManagedHandler)
}

fn accept_config(_: &conduit_panel::Node) -> Result<(), ResolutionError> {
    Ok(())
}

fn satisfaction_fact(value: &str) -> SemanticHash {
    SemanticHash::from_bytes(sha2::Sha256::digest(value.as_bytes()).into())
}

fn identity() -> ManagedComponentIdentity {
    ManagedComponentIdentity {
        component: "server".to_owned(),
        semantic_contract: "conduit.host/net/http/listen".to_owned(),
        implementation_id: "conduit/http-linux-listener".to_owned(),
        implementation_version: "1".to_owned(),
        implementation_identity: "sha256:implementation".to_owned(),
        artifacts: vec![ManagedArtifactIdentity {
            id: "conduit/http-linux-listener-artifact".to_owned(),
            digest: "sha256:artifact".to_owned(),
        }],
        host_id: "host/local".to_owned(),
        host_boot_id: "boot/7".to_owned(),
        host_observation_id: "observation/host-7".to_owned(),
        run_id: "run/managed".to_owned(),
        plan_identity: "sha256:plan".to_owned(),
        plan_epoch: 4,
        activation_generation: 3,
        resources: vec!["resource/listener-8080".to_owned()],
        grants: vec!["grant/listen".to_owned()],
        leases: vec!["lease/listener-8080".to_owned()],
    }
}

fn authority(actions: Vec<ManagedLifecycleAction>) -> ManagedLifecycleAuthority {
    ManagedLifecycleAuthority {
        requester: "operator/test".to_owned(),
        authority_id: "grant/lifecycle".to_owned(),
        provider: ManagedProviderAvailability::Available,
        grant: ManagedGrantState::Active,
        resources: ManagedResourceState::Available,
        leases: ManagedLeaseState::Current,
        not_before_tick: 0,
        expires_at_tick: 1_000,
        actions,
        inhibit_asserted: false,
    }
}

fn request(
    machine: &ManagedComponentMachine,
    id: &str,
    action: ManagedLifecycleAction,
    tick: u64,
) -> ManagedLifecycleRequest {
    ManagedLifecycleRequest {
        schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
        request_id: id.to_owned(),
        component: machine.observation().identity.component.clone(),
        action,
        expected_plan_epoch: machine.observation().identity.plan_epoch,
        expected_activation_generation: machine.observation().identity.activation_generation,
        expected_observation_sequence: machine.observation().sequence,
        issued_at_tick: tick,
        deadline_tick: tick + 100,
        causation: format!("test/{id}"),
    }
}

fn machine() -> ManagedComponentMachine {
    ManagedComponentMachine::new(
        ManagedComponentDescriptor::full_standing_service(ManagedAdapterBoundary::Native),
        identity(),
        10,
        900,
    )
    .unwrap()
}

fn prepare(machine: &mut ManagedComponentMachine, tick: u64) {
    let request = request(machine, "prepare", ManagedLifecycleAction::Prepare, tick);
    machine
        .request(
            request,
            &authority(vec![ManagedLifecycleAction::Prepare]),
            tick,
        )
        .unwrap();
    machine
        .apply_provider_event(
            "prepare",
            ManagedProviderEvent::Prepared {
                resource_evidence: vec!["resource/listener-8080".to_owned()],
            },
            tick + 1,
        )
        .unwrap();
}

fn activate(machine: &mut ManagedComponentMachine, tick: u64) {
    let request = request(machine, "activate", ManagedLifecycleAction::Activate, tick);
    machine
        .request(
            request,
            &authority(vec![ManagedLifecycleAction::Activate]),
            tick,
        )
        .unwrap();
    machine
        .apply_provider_event("activate", ManagedProviderEvent::Activated, tick + 1)
        .unwrap();
}

#[test]
fn full_service_lifecycle_keeps_active_distinct_from_waiting() {
    let mut machine = machine();
    prepare(&mut machine, 20);
    activate(&mut machine, 30);
    assert_eq!(machine.observation().state, ManagedLifecycleState::Active);
    assert_eq!(
        machine.observation().readiness,
        ManagedRuntimeReadiness::Waiting
    );

    machine
        .set_readiness(ManagedRuntimeReadiness::Ready, 32, "request-admitted")
        .unwrap();
    machine
        .set_readiness(ManagedRuntimeReadiness::Waiting, 33, "request-complete")
        .unwrap();

    let stop = request(&machine, "stop", ManagedLifecycleAction::Stop, 40);
    let stop_authority = authority(vec![ManagedLifecycleAction::Stop]);
    machine.request(stop, &stop_authority, 40).unwrap();
    machine
        .apply_provider_event(
            "stop",
            ManagedProviderEvent::AdmissionClosed { in_flight: 1 },
            41,
        )
        .unwrap();
    assert_eq!(
        machine.observation().state,
        ManagedLifecycleState::Quiescing
    );
    machine
        .apply_provider_event(
            "stop",
            ManagedProviderEvent::Progress {
                progress: ManagedLifecycleProgress {
                    completed_units: 1,
                    total_units: Some(1),
                    detail: "admitted request drained".to_owned(),
                },
            },
            42,
        )
        .unwrap();
    assert_eq!(
        machine.observation().state,
        ManagedLifecycleState::Quiescing,
        "progress is evidence, not completion"
    );
    machine
        .apply_provider_event(
            "stop",
            ManagedProviderEvent::Quiesced {
                drained: 1,
                cancelled: 0,
            },
            43,
        )
        .unwrap();
    machine
        .apply_provider_event("stop", ManagedProviderEvent::CleanupStarted, 44)
        .unwrap();
    machine
        .apply_provider_event(
            "stop",
            ManagedProviderEvent::CleanupComplete {
                released_resources: vec!["resource/listener-8080".to_owned()],
            },
            45,
        )
        .unwrap();

    assert_eq!(machine.observation().state, ManagedLifecycleState::Stopped);
    assert_eq!(machine.observation().cleanup, ManagedCleanupState::Complete);
    assert!(machine.evidence().any(|event| {
        event.reason == ManagedLifecycleReason::AdmissionClosed
            && event.state == ManagedLifecycleState::Quiescing
    }));
}

#[test]
fn requests_are_authorized_fenced_and_idempotent_without_proving_transition() {
    let mut machine = machine();
    let prepare_request = request(
        &machine,
        "prepare-duplicate",
        ManagedLifecycleAction::Prepare,
        20,
    );
    let prepare_authority = authority(vec![ManagedLifecycleAction::Prepare]);
    let receipt = machine
        .request(prepare_request.clone(), &prepare_authority, 20)
        .unwrap();
    assert!(!receipt.duplicate);
    assert_eq!(
        machine.observation().state,
        ManagedLifecycleState::Configured,
        "request acceptance is not a provider commit"
    );
    let duplicate = machine
        .request(prepare_request, &prepare_authority, 20)
        .unwrap();
    assert!(duplicate.duplicate);

    let mut stale = request(&machine, "stale", ManagedLifecycleAction::Prepare, 21);
    stale.expected_plan_epoch -= 1;
    assert_eq!(
        machine
            .request(stale, &prepare_authority, 21)
            .unwrap_err()
            .code,
        ManagedLifecycleReason::StalePlanEpoch.code()
    );

    let mut wrong_generation = request(
        &machine,
        "wrong-generation",
        ManagedLifecycleAction::Prepare,
        21,
    );
    wrong_generation.expected_activation_generation += 1;
    assert_eq!(
        machine
            .request(wrong_generation, &prepare_authority, 21)
            .unwrap_err()
            .code,
        ManagedLifecycleReason::WrongGeneration.code()
    );
}

#[test]
fn independent_inhibit_and_missing_facets_fail_closed() {
    let mut machine = machine();
    let activate = request(
        &machine,
        "activate-too-early",
        ManagedLifecycleAction::Activate,
        20,
    );
    assert_eq!(
        machine
            .request(
                activate,
                &authority(vec![ManagedLifecycleAction::Activate]),
                20,
            )
            .unwrap_err()
            .code,
        ManagedLifecycleReason::WrongState.code()
    );

    let prepare = request(
        &machine,
        "inhibited-prepare",
        ManagedLifecycleAction::Prepare,
        20,
    );
    let mut inhibited = authority(vec![ManagedLifecycleAction::Prepare]);
    inhibited.inhibit_asserted = true;
    assert_eq!(
        machine.request(prepare, &inhibited, 20).unwrap_err().code,
        ManagedLifecycleReason::InhibitAsserted.code()
    );

    let descriptor = ManagedComponentDescriptor::new(
        "conduit.lifecycle/prepare-only-ffi",
        ManagedAdapterBoundary::FfiFirmware,
        ManagedLifecycleFacets {
            prepare: true,
            activate: false,
            quiesce: false,
            retained_prepared_state: false,
            cleanup: false,
            bounded_cancellation: false,
            progress: false,
        },
        8,
        0,
        20,
        "public-operational",
    )
    .unwrap();
    let mut prepare_only = ManagedComponentMachine::new(descriptor, identity(), 10, 100).unwrap();
    let mut stop = request(
        &prepare_only,
        "unsupported-stop",
        ManagedLifecycleAction::Stop,
        20,
    );
    stop.deadline_tick = 30;
    assert_eq!(
        prepare_only
            .request(stop, &authority(vec![ManagedLifecycleAction::Stop]), 20,)
            .unwrap_err()
            .code,
        ManagedLifecycleReason::UnsupportedFacet.code()
    );
}

#[test]
fn host_loss_never_fabricates_cleanup_or_stopped() {
    let mut machine = machine();
    prepare(&mut machine, 20);
    activate(&mut machine, 30);
    machine.report_loss(true, 40, "host-observation-lost");
    assert_eq!(machine.observation().state, ManagedLifecycleState::Failed);
    assert_eq!(
        machine.observation().cleanup,
        ManagedCleanupState::Unprovable
    );
    assert_eq!(
        machine.observation().reason,
        ManagedLifecycleReason::HostLost
    );

    machine.retire_for_plan_replacement(41, "candidate-epoch-5-committed");
    assert!(machine.observation().retired);
    assert_eq!(machine.observation().state, ManagedLifecycleState::Failed);
    assert_eq!(
        machine
            .apply_provider_event("late", ManagedProviderEvent::Activated, 42)
            .unwrap_err()
            .code,
        ManagedLifecycleReason::RetiredGenerationWake.code()
    );
}

#[test]
fn cleanup_deadline_reports_failure_separately_from_cleanup_disposition() {
    let mut machine = machine();
    prepare(&mut machine, 20);
    let clean = request(&machine, "clean", ManagedLifecycleAction::Clean, 30);
    machine
        .request(clean, &authority(vec![ManagedLifecycleAction::Clean]), 30)
        .unwrap();
    machine
        .apply_provider_event("clean", ManagedProviderEvent::CleanupStarted, 31)
        .unwrap();
    let deadline = machine.check_deadline(131).unwrap_err();
    assert_eq!(deadline.code, ManagedLifecycleReason::CleanupTimeout.code());
    assert_eq!(machine.observation().state, ManagedLifecycleState::Failed);
    assert_eq!(machine.observation().cleanup, ManagedCleanupState::TimedOut);
}

#[test]
fn evidence_retention_is_finite_and_reports_its_earliest_sequence() {
    let descriptor = ManagedComponentDescriptor::new(
        "conduit.lifecycle/small-window",
        ManagedAdapterBoundary::Deterministic,
        ManagedLifecycleFacets::full(),
        4,
        2,
        100,
        "public-operational",
    )
    .unwrap();
    let mut machine = ManagedComponentMachine::new(descriptor, identity(), 10, 900).unwrap();
    prepare(&mut machine, 20);
    activate(&mut machine, 30);
    machine
        .set_readiness(ManagedRuntimeReadiness::Ready, 32, "ready")
        .unwrap();
    machine
        .set_readiness(ManagedRuntimeReadiness::Waiting, 33, "waiting")
        .unwrap();
    assert_eq!(machine.evidence().count(), 4);
    assert!(machine.earliest_evidence_sequence() > 1);
}

#[test]
fn admission_rejections_preserve_exact_reason_categories() {
    type MutateAuthority = fn(&mut ManagedLifecycleAuthority);
    let cases: [(ManagedLifecycleReason, MutateAuthority); 5] = [
        (
            ManagedLifecycleReason::UnavailableImplementation,
            |authority: &mut ManagedLifecycleAuthority| {
                authority.provider = ManagedProviderAvailability::Unavailable;
            },
        ),
        (ManagedLifecycleReason::DeniedGrant, |authority| {
            authority.grant = ManagedGrantState::Denied;
        }),
        (ManagedLifecycleReason::RevokedGrant, |authority| {
            authority.grant = ManagedGrantState::Revoked;
        }),
        (ManagedLifecycleReason::ResourceConflict, |authority| {
            authority.resources = ManagedResourceState::Conflict;
        }),
        (ManagedLifecycleReason::ExpiredLease, |authority| {
            authority.leases = ManagedLeaseState::Expired;
        }),
    ];
    for (expected, mutate) in cases {
        let mut machine = machine();
        let request = request(
            &machine,
            expected.code(),
            ManagedLifecycleAction::Prepare,
            20,
        );
        let mut authority = authority(vec![ManagedLifecycleAction::Prepare]);
        mutate(&mut authority);
        let error = machine.request(request, &authority, 20).unwrap_err();
        assert_eq!(error.reason, expected);
        assert_eq!(
            machine.observation().state,
            ManagedLifecycleState::Configured
        );
    }

    let mut stale_sequence = machine();
    let mut stale = request(
        &stale_sequence,
        "stale-sequence",
        ManagedLifecycleAction::Prepare,
        20,
    );
    stale.expected_observation_sequence += 1;
    assert_eq!(
        stale_sequence
            .request(stale, &authority(vec![ManagedLifecycleAction::Prepare]), 20)
            .unwrap_err()
            .reason,
        ManagedLifecycleReason::StaleRequest
    );

    let mut stale_host = machine();
    let stale = request(
        &stale_host,
        "stale-host",
        ManagedLifecycleAction::Prepare,
        901,
    );
    assert_eq!(
        stale_host
            .request(
                stale,
                &authority(vec![ManagedLifecycleAction::Prepare]),
                901
            )
            .unwrap_err()
            .reason,
        ManagedLifecycleReason::StaleHostFact
    );
}

#[test]
fn provider_failures_cancellation_and_deadlines_never_fabricate_cleanup() {
    let mut preparation = machine();
    let prepare_request = request(
        &preparation,
        "preparation-fails",
        ManagedLifecycleAction::Prepare,
        20,
    );
    preparation
        .request(
            prepare_request,
            &authority(vec![ManagedLifecycleAction::Prepare]),
            20,
        )
        .unwrap();
    preparation
        .apply_provider_event(
            "preparation-fails",
            ManagedProviderEvent::Failed {
                reason: ManagedLifecycleReason::PreparationFailed,
                cleanup: ManagedCleanupState::Unprovable,
            },
            21,
        )
        .unwrap();
    assert_eq!(
        preparation.observation().state,
        ManagedLifecycleState::Failed
    );
    assert_eq!(
        preparation.observation().cleanup,
        ManagedCleanupState::Unprovable
    );

    let mut provider_loss = machine();
    prepare(&mut provider_loss, 20);
    activate(&mut provider_loss, 30);
    provider_loss.report_loss(false, 40, "provider-disappeared");
    assert_eq!(
        provider_loss.observation().reason,
        ManagedLifecycleReason::ProviderLost
    );
    assert_eq!(
        provider_loss.observation().cleanup,
        ManagedCleanupState::Required
    );

    let mut drain = machine();
    prepare(&mut drain, 20);
    activate(&mut drain, 30);
    let stop = request(&drain, "drain-deadline", ManagedLifecycleAction::Stop, 40);
    drain
        .request(stop, &authority(vec![ManagedLifecycleAction::Stop]), 40)
        .unwrap();
    drain
        .apply_provider_event(
            "drain-deadline",
            ManagedProviderEvent::AdmissionClosed { in_flight: 2 },
            41,
        )
        .unwrap();
    assert_eq!(
        drain.check_deadline(141).unwrap_err().reason,
        ManagedLifecycleReason::DrainDeadline
    );
    assert_eq!(drain.observation().cleanup, ManagedCleanupState::Required);

    let mut cancelled = machine();
    prepare(&mut cancelled, 20);
    activate(&mut cancelled, 30);
    let stop = request(&cancelled, "cancel-stop", ManagedLifecycleAction::Stop, 40);
    cancelled
        .request(stop, &authority(vec![ManagedLifecycleAction::Stop]), 40)
        .unwrap();
    cancelled
        .apply_provider_event(
            "cancel-stop",
            ManagedProviderEvent::AdmissionClosed { in_flight: 1 },
            41,
        )
        .unwrap();
    cancelled.cancel_request("cancel-stop", 42).unwrap();
    assert_eq!(cancelled.observation().state, ManagedLifecycleState::Failed);
    assert_eq!(
        cancelled.observation().cleanup,
        ManagedCleanupState::Required
    );
}

#[test]
fn every_adapter_boundary_uses_the_same_explicit_facet_contract() {
    for (boundary, name) in [
        (ManagedAdapterBoundary::Native, "native"),
        (ManagedAdapterBoundary::Wasm, "wasm"),
        (
            ManagedAdapterBoundary::SupervisedProcess,
            "supervised-process",
        ),
        (ManagedAdapterBoundary::FfiFirmware, "ffi-firmware"),
        (ManagedAdapterBoundary::Remote, "remote"),
        (ManagedAdapterBoundary::Deterministic, "deterministic"),
    ] {
        let descriptor = ManagedComponentDescriptor::new(
            format!("conduit.lifecycle/{name}"),
            boundary,
            ManagedLifecycleFacets {
                prepare: true,
                activate: true,
                quiesce: boundary != ManagedAdapterBoundary::Remote,
                retained_prepared_state: false,
                cleanup: boundary != ManagedAdapterBoundary::Remote,
                bounded_cancellation: boundary != ManagedAdapterBoundary::Remote,
                progress: false,
            },
            8,
            0,
            20,
            "public-operational",
        )
        .unwrap();
        assert_eq!(descriptor.boundary, boundary);
        assert_eq!(
            descriptor.supports(ManagedLifecycleAction::Stop),
            boundary != ManagedAdapterBoundary::Remote
        );
    }
}

#[test]
fn native_and_deterministic_adapters_normalize_lifecycle_evidence() {
    fn evidence(
        boundary: ManagedAdapterBoundary,
    ) -> Vec<(
        ManagedLifecycleState,
        ManagedRuntimeReadiness,
        ManagedCleanupState,
        ManagedLifecycleReason,
    )> {
        let descriptor = ManagedComponentDescriptor::new(
            "conduit.lifecycle/normalized-service",
            boundary,
            ManagedLifecycleFacets::full(),
            64,
            32,
            100,
            "public-operational",
        )
        .unwrap();
        let mut machine = ManagedComponentMachine::new(descriptor, identity(), 10, 900).unwrap();
        prepare(&mut machine, 20);
        activate(&mut machine, 30);
        machine
            .set_readiness(ManagedRuntimeReadiness::Ready, 32, "ready")
            .unwrap();
        machine
            .set_readiness(ManagedRuntimeReadiness::Waiting, 33, "waiting")
            .unwrap();
        machine
            .evidence()
            .map(|event| (event.state, event.readiness, event.cleanup, event.reason))
            .collect()
    }
    assert_eq!(
        evidence(ManagedAdapterBoundary::Native),
        evidence(ManagedAdapterBoundary::Deterministic)
    );
}

#[test]
fn cancellation_cleanup_failure_and_plan_replacement_cover_each_commit_stage() {
    let mut before_commit = machine();
    let pending = request(
        &before_commit,
        "cancel-before-commit",
        ManagedLifecycleAction::Prepare,
        20,
    );
    before_commit
        .request(
            pending,
            &authority(vec![ManagedLifecycleAction::Prepare]),
            20,
        )
        .unwrap();
    before_commit
        .cancel_request("cancel-before-commit", 21)
        .unwrap();
    assert_eq!(
        before_commit.observation().state,
        ManagedLifecycleState::Configured
    );

    let mut cleanup_failure = machine();
    prepare(&mut cleanup_failure, 20);
    let clean = request(
        &cleanup_failure,
        "cleanup-failure",
        ManagedLifecycleAction::Clean,
        30,
    );
    cleanup_failure
        .request(clean, &authority(vec![ManagedLifecycleAction::Clean]), 30)
        .unwrap();
    cleanup_failure
        .apply_provider_event("cleanup-failure", ManagedProviderEvent::CleanupStarted, 31)
        .unwrap();
    cleanup_failure
        .apply_provider_event(
            "cleanup-failure",
            ManagedProviderEvent::Failed {
                reason: ManagedLifecycleReason::CleanupFailed,
                cleanup: ManagedCleanupState::Failed,
            },
            32,
        )
        .unwrap();
    assert_eq!(
        cleanup_failure.observation().state,
        ManagedLifecycleState::Failed
    );
    assert_eq!(
        cleanup_failure.observation().cleanup,
        ManagedCleanupState::Failed
    );

    for stage in 0..5 {
        let mut staged = machine();
        if stage >= 1 {
            prepare(&mut staged, 20);
        }
        if stage >= 2 {
            activate(&mut staged, 30);
        }
        if stage >= 3 {
            let stop = request(&staged, "retire-stop", ManagedLifecycleAction::Stop, 40);
            staged
                .request(stop, &authority(vec![ManagedLifecycleAction::Stop]), 40)
                .unwrap();
            staged
                .apply_provider_event(
                    "retire-stop",
                    ManagedProviderEvent::AdmissionClosed { in_flight: 1 },
                    41,
                )
                .unwrap();
        }
        if stage >= 4 {
            staged
                .apply_provider_event(
                    "retire-stop",
                    ManagedProviderEvent::Quiesced {
                        drained: 1,
                        cancelled: 0,
                    },
                    42,
                )
                .unwrap();
            staged
                .apply_provider_event("retire-stop", ManagedProviderEvent::CleanupStarted, 43)
                .unwrap();
        }
        let state_before_retirement = staged.observation().state;
        staged.retire_for_plan_replacement(50, format!("replacement-stage-{stage}"));
        assert!(staged.observation().retired);
        assert_eq!(staged.observation().state, state_before_retirement);
        assert_eq!(
            staged
                .apply_provider_event("retired", ManagedProviderEvent::Activated, 51)
                .unwrap_err()
                .reason,
            ManagedLifecycleReason::RetiredGenerationWake
        );
    }
}

#[test]
fn fixture_inventory_and_ci_composition_keep_components_work_and_shards_distinct() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../conformance/c4/managed-component-lifecycle.json"
    ))
    .unwrap();
    let ids = fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| case["id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    for required in [
        "successful-full-lifecycle",
        "activation-from-wrong-state",
        "duplicate-request",
        "stale-request",
        "stale-plan-epoch",
        "wrong-component-generation",
        "unavailable-implementation",
        "stale-host-fact",
        "denied-grant",
        "revoked-grant",
        "resource-conflict",
        "expired-lease",
        "failed-preparation",
        "active-while-scheduler-waiting",
        "quiesce-with-in-flight-work",
        "new-admission-rejected-during-quiescence",
        "drain-deadline",
        "abort",
        "provider-loss",
        "host-loss-unprovable-cleanup",
        "cleanup-success",
        "cleanup-failure",
        "cleanup-timeout",
        "inhibit-assertion",
        "plan-replacement-every-stage",
        "unsupported-lifecycle-facet",
        "late-callback-from-retired-generation",
        "deterministic-native-normalized-evidence",
        "structural-satisfaction-distinct-profiles",
        "leased-audio-production-binding",
        "network-listener-production-binding",
        "ci-obligation-uses-managed-server",
        "typed-inspection-keeps-dimensions",
    ] {
        assert!(
            ids.contains(required),
            "missing managed fixture `{required}`"
        );
    }
    assert_eq!(
        fixture["ci_composition"]["managed_components"],
        serde_json::json!(["tour-server"])
    );
    assert_eq!(
        fixture["ci_composition"]["work_obligations"],
        serde_json::json!(["browser-test"])
    );
    assert_eq!(
        fixture["ci_composition"]["planning_assignments"],
        serde_json::json!(["ci-shard"])
    );
}

#[test]
fn generic_installed_implementation_hashes_and_exposes_the_optional_interface() {
    let descriptor =
        ManagedComponentDescriptor::full_standing_service(ManagedAdapterBoundary::Remote);
    let descriptor_hash = descriptor.semantic_hash().unwrap();
    let bytes = b"remote-http-listener-adapter";
    let digest = ArtifactDigest::from_bytes(sha2::Sha256::digest(bytes).into());
    let profile = "conduit/remote-http-listener-profile";
    let mut registry = Registry::default();
    registry
        .register_installed_implementation(InstalledImplementationRegistration {
            contract: &HTTP_LISTENER_CONTRACT,
            implementation_id: "example.remote/http-listener".to_owned(),
            implementation_version: "observed-build-a".to_owned(),
            executor: ExecutorKind::RemoteEndpoint,
            entrypoint_name: "serve".to_owned(),
            entrypoint_adapter: "example.remote/message-step".to_owned(),
            entrypoint_abi: "example.remote/protocol".to_owned(),
            entrypoint_protocol_version: 0,
            execution_profile: PinnedDescriptor {
                id: Id(profile),
                schema_version: 0,
                semantic_hash: SemanticHash::from_bytes(sha2::Sha256::digest(profile).into()),
            },
            artifacts: vec![InstalledArtifactRegistration {
                id: "example.remote/http-listener-artifact".to_owned(),
                digest,
                media_type: "application/vnd.example.remote-adapter".to_owned(),
                byte_size: bytes.len() as u64,
                target: None,
                abi: Some("example.remote/protocol".to_owned()),
                builder: "example.remote/builder".to_owned(),
                source_digest: digest,
                build_recipe_digest: digest,
                reproducible: true,
                license_expressions: Vec::new(),
                role: "adapter".to_owned(),
                required: true,
            }],
            required_capabilities: Vec::new(),
            required_authorities: Vec::new(),
            required_effects: Vec::new(),
            minimum_plan_version: 0,
            maximum_plan_version: u32::MAX,
            minimum_runtime_protocol: 1,
            maximum_runtime_protocol: 1,
            coexistence_memory_bytes: 0,
            managed_lifecycle: Some(descriptor.clone()),
            factory: noop_factory,
            validate_config: accept_config,
        })
        .unwrap();
    registry
        .register_managed_compiled_in_host_service(
            CompiledInHostService {
                contract: &HTTP_LISTENER_CONTRACT,
                implementation_id: "example.native/http-listener",
                artifact_id: "example.native/http-listener-artifact",
                entrypoint: "serve",
                source_bytes: b"native-http-listener-adapter",
                required_authorities: &[],
                factory: noop_factory,
                validate_config: accept_config,
            },
            ManagedComponentDescriptor::full_standing_service(ManagedAdapterBoundary::Native),
        )
        .unwrap();
    let installed = registry
        .installed_providers()
        .into_iter()
        .find(|provider| provider.manifest.id.as_str() == "example.remote/http-listener")
        .unwrap();
    assert_eq!(installed.managed_lifecycle, Some(&descriptor));
    assert_eq!(installed.manifest.provided_interfaces.len(), 2);
    assert_eq!(
        installed.manifest.provided_interfaces[0]
            .interface
            .id
            .as_str(),
        conduit_runtime::MANAGED_COMPONENT_INTERFACE_ID
    );
    assert_eq!(
        installed.manifest.provided_interfaces[0]
            .interface
            .semantic_hash,
        conduit_runtime::managed_component_interface_hash()
    );
    assert_eq!(
        installed.manifest.provided_interfaces[1]
            .interface
            .semantic_hash,
        descriptor_hash
    );
    assert_eq!(
        installed.manifest.provided_interfaces[0]
            .entrypoint
            .as_str(),
        "serve"
    );
    let providers = registry
        .installed_providers()
        .into_iter()
        .filter(|provider| provider.contract.id == HTTP_LISTENER_CONTRACT.id)
        .collect::<Vec<_>>();
    assert_eq!(providers.len(), 2);
    assert!(providers.iter().all(|provider| {
        provider.manifest.provided_interfaces[0]
            .interface
            .id
            .as_str()
            == conduit_runtime::MANAGED_COMPONENT_INTERFACE_ID
            && provider.manifest.provided_interfaces[0]
                .interface
                .semantic_hash
                == conduit_runtime::managed_component_interface_hash()
    }));

    let facet_ids = [
        "conduit.lifecycle/prepare",
        "conduit.lifecycle/activate",
        "conduit.lifecycle/quiesce",
        "conduit.lifecycle/retained-prepared-state",
        "conduit.lifecycle/cleanup",
        "conduit.lifecycle/bounded-cancellation",
        "conduit.lifecycle/progress",
    ];
    let mut proof_identities = Vec::new();
    for provider in &providers {
        let offered_profile = provider
            .managed_lifecycle
            .expect("both test implementations offer managed lifecycle");
        assert_eq!(offered_profile.facets, ManagedLifecycleFacets::full());
        let supported = satisfaction_fact("conduit.lifecycle/facet-supported");
        let facets = facet_ids.map(|id| SatisfactionFacet {
            id: Id(id),
            required_hash: supported,
            offered_hash: supported,
        });
        let common = satisfaction_fact("conduit.lifecycle/obligation-satisfied");
        let obligations = [
            SatisfactionObligation {
                id: Id("semantic-contract"),
                required_hash: provider.manifest.semantic_contract.semantic_hash,
                offered_hash: provider.manifest.semantic_contract.semantic_hash,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/contract-satisfied"),
            },
            SatisfactionObligation {
                id: Id("ports"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/ports-unmodified"),
            },
            SatisfactionObligation {
                id: Id("configuration"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/configuration-satisfied"),
            },
            SatisfactionObligation {
                id: Id("representation"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/representation-satisfied"),
            },
            SatisfactionObligation {
                id: Id("ownership-lifetime"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/ownership-satisfied"),
            },
            SatisfactionObligation {
                id: Id("lifecycle"),
                required_hash: conduit_runtime::managed_component_interface_hash(),
                offered_hash: offered_profile.semantic_hash().unwrap(),
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/required-facets-satisfied"),
            },
            SatisfactionObligation {
                id: Id("authority"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/authority-external"),
            },
            SatisfactionObligation {
                id: Id("resources"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/resources-exact"),
            },
            SatisfactionObligation {
                id: Id("boundedness"),
                required_hash: common,
                offered_hash: common,
                outcome: CompatibilityOutcome::Compatible,
                reason: Id("conduit.lifecycle/bounds-satisfied"),
            },
        ];
        let mut proof = SatisfactionProof {
            schema_version: 0,
            identity: SemanticHash::from_bytes([0; 32]),
            role: SatisfactionRole::Implementation,
            method: SatisfactionMethod::StructuralFacets,
            required: DescriptorRef {
                kind: provider.manifest.semantic_contract.id,
                schema_version: provider.manifest.semantic_contract.schema_version,
                semantic_hash: provider.manifest.semantic_contract.semantic_hash,
            },
            offered: DescriptorRef {
                kind: provider.manifest.id,
                schema_version: provider.manifest.schema_version,
                semantic_hash: provider.manifest.identity,
            },
            provider: Some(SatisfactionPin {
                descriptor: DescriptorRef {
                    kind: Id("conduit.lifecycle/managed-component-satisfaction"),
                    schema_version: 0,
                    semantic_hash: satisfaction_fact(
                        "conduit.lifecycle/managed-component-satisfaction",
                    ),
                },
            }),
            provider_rule: Some(Id("conduit.lifecycle/structural-facets")),
            policy: None,
            facets: &facets,
            obligations: &obligations,
            outcome: CompatibilityOutcome::Compatible,
            reason: SatisfactionReason::Satisfied,
            explanation: Id("conduit.lifecycle/implementation-satisfied"),
            explicit_requirement: ExplicitSatisfactionRequirement::None,
        };
        let mut scratch = vec![SemanticHash::from_bytes([0; 32]); proof.identity_fact_count()];
        proof.identity = proof.semantic_hash(&mut scratch).unwrap();
        validate_satisfaction_proof(&proof, &mut scratch).unwrap();
        proof_identities.push(proof.identity);
    }
    assert_ne!(
        proof_identities[0], proof_identities[1],
        "the same semantic contract is satisfied by distinct exact implementation proofs"
    );
    assert_ne!(
        providers[0].manifest.provided_interfaces[1]
            .interface
            .semantic_hash,
        providers[1].manifest.provided_interfaces[1]
            .interface
            .semantic_hash
    );
}
