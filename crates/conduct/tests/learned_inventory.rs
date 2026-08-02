use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use conduit_compile::{
    InstalledProfile, compile_source, fixture_host_service_authority_observation,
    observed_host_service_constraints,
};
use conduit_core::{
    Id, PlanValidationContext, ReadyQueueDiscipline, SCHEDULER_CONTRACT_VERSION, SchedulerPolicy,
    StopPolicy,
};
use conduit_learned::{
    InferenceProviderFault,
    lifecycle::{
        PROMOTION_AUTHORITY_CONSTRAINTS, PromotionFixtureFault,
        register_deterministic_lifecycle_fixture_provider,
        register_deterministic_lifecycle_fixture_provider_with_promotion_fault,
        register_deterministic_training_provider,
    },
    register_deterministic_inference_provider,
    register_deterministic_inference_provider_with_fault, register_learned_contracts,
};
use conduit_runtime::{
    AvailabilityState, ExactExecutionReport, ExactRunContext, Registry, RunIo, RuntimeError,
    SchedulerReservation,
};

#[derive(Clone, Copy)]
enum PromotionUseFault {
    None,
    Revoked,
    WrongResource,
    WrongResourceGeneration,
}

fn promotion_observation(
    run_id: &str,
    epoch: u64,
) -> conduit_compile::ObservedHostServiceAuthority {
    let constraints = observed_host_service_constraints(&PROMOTION_AUTHORITY_CONSTRAINTS);
    fixture_host_service_authority_observation(
        "learned/promote",
        "root/promote",
        run_id,
        epoch,
        &constraints,
    )
    .unwrap()
}

fn execute_promotion_fixture(
    provider_fault: PromotionFixtureFault,
    use_fault: PromotionUseFault,
    planned_run: &str,
    planned_epoch: u64,
    actual_run: &str,
    actual_epoch: u64,
    cancel: bool,
) -> Result<(ExactExecutionReport, Vec<u8>), RuntimeError> {
    let source = include_str!("../../../examples/learned-lifecycle.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_lifecycle_fixture_provider_with_promotion_fault(
        &mut registry,
        provider_fault,
    )
    .unwrap();
    let authorities = if planned_run.is_empty() {
        Vec::new()
    } else {
        vec![promotion_observation(planned_run, planned_epoch)]
    };
    let mut installed =
        InstalledProfile::observe_registry_with_host_authorities(source, &registry, &authorities)
            .unwrap();
    if matches!(use_fault, PromotionUseFault::WrongResourceGeneration) {
        let authority = installed
            .input
            .candidates
            .iter_mut()
            .find(|candidate| candidate.implementation.semantic_contract.id == "learned/promote")
            .and_then(|candidate| candidate.authorities.first_mut())
            .unwrap();
        for constraints in [
            &mut authority.effect.constraints,
            &mut authority.grant.constraints,
        ] {
            constraints
                .iter_mut()
                .find(|constraint| {
                    constraint.id
                        == conduit_learned::lifecycle::PROMOTION_RESOURCE_GENERATION_CONSTRAINT
                })
                .unwrap()
                .semantic_hash = conduit_core::SemanticHash::from_bytes([0x99; 32]).to_string();
        }
        installed.input.seal().unwrap();
    }
    let document = compile_source(source, &installed.input).unwrap();
    if matches!(
        use_fault,
        PromotionUseFault::Revoked | PromotionUseFault::WrongResource
    ) {
        let authority = installed
            .input
            .candidates
            .iter_mut()
            .find(|candidate| candidate.implementation.semantic_contract.id == "learned/promote")
            .and_then(|candidate| candidate.authorities.first_mut())
            .unwrap();
        match use_fault {
            PromotionUseFault::Revoked => authority.status = "revoked".to_owned(),
            PromotionUseFault::WrongResource => {
                authority.resource_lease.resource_binding =
                    "conduit.resource/learned-wrong-slot".to_owned();
            }
            PromotionUseFault::None | PromotionUseFault::WrongResourceGeneration => unreachable!(),
        }
    }
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let panel = conduit_panel::parse(source).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan)?;
    let grant_observations = installed.grant_observations(&plan)?;
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let context = ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: actual_epoch,
        run_id: Id(actual_run),
        grant_observations: &grant_observations,
        validation: PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 256,
            max_tick: 512,
            max_consecutive_yields: 8,
            max_events: 256,
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    };
    let mut io = RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
        display: &mut display,
    };
    let report = if cancel {
        resolved.cancel_exact_report(&plan, &bindings, context, StopPolicy::Abort, &mut io)?
    } else {
        resolved.run_exact_report(&plan, &bindings, context, &mut io)?
    };
    Ok((report, display))
}

fn execute_inference_fixture(
    fault: InferenceProviderFault,
    cancel: bool,
) -> Result<(ExactExecutionReport, Vec<u8>), RuntimeError> {
    let source = include_str!("../../../examples/learned-fixed-inference.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider_with_fault(&mut registry, fault).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let arena = bumpalo::Bump::new();
    let plan = document.as_plan(&arena).unwrap();
    let panel = conduit_panel::parse(source).unwrap();
    let resolved = registry.resolve(&panel).unwrap();
    let bindings = installed.bindings(&plan)?;
    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut display = Vec::new();
    let context = ExactRunContext {
        semantic_source_hash: plan.source_semantic_hash,
        plan_epoch: 1,
        run_id: Id("conduit/conduct-run"),
        grant_observations: &[],
        validation: PlanValidationContext {
            supported_schema_version: plan.schema_version,
            now: plan.created_at,
        },
        scheduler_policy: SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 64,
            max_tick: 128,
            max_consecutive_yields: 8,
            max_events: 128,
        },
        reservation: SchedulerReservation {
            available_runtime_memory_bytes: plan.budget.memory_bytes,
            executor_overhead_limit_bytes: plan.budget.memory_bytes,
        },
    };
    let mut io = RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
        display: &mut display,
    };
    let report = if cancel {
        resolved.cancel_exact_report(&plan, &bindings, context, StopPolicy::Abort, &mut io)?
    } else {
        resolved.run_exact_report(&plan, &bindings, context, &mut io)?
    };
    Ok((report, display))
}

fn workspace_file(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

#[test]
fn exact_inference_binding_runs_through_the_production_executor() {
    let source = include_str!("../../../examples/learned-fixed-inference.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let inference = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "learned/infer")
        .unwrap();
    assert_eq!(
        inference.implementation.id,
        "conduit.learned/fixed-linear-rust"
    );
    assert_eq!(
        inference.artifact,
        "conduit.learned/fixed-linear-rust-artifact"
    );

    for (path, expected) in [
        (
            "examples/learned-fixed-inference.panel",
            "learned:i16:1x2:[35,-3]",
        ),
        (
            "examples/learned-inference-compose.panel",
            "LEARNED:I16:1X2:[35,-3]",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn inference_provider_loss_and_cancellation_use_the_production_binding() {
    let error = execute_inference_fixture(InferenceProviderFault::ProviderLost, false).unwrap_err();
    assert_eq!(error.code, "CND-LEARN-009");

    let (report, display) = execute_inference_fixture(InferenceProviderFault::None, true).unwrap();
    assert_eq!(report.summary.nodes_completed, 0);
    assert!(display.is_empty(), "cancelled inference emitted a result");
}

#[test]
fn inference_contract_only_missing_and_stale_provider_states_are_distinct() {
    let source = include_str!("../../../examples/learned-fixed-inference.panel");
    let panel = conduit_panel::parse(source).unwrap();
    let mut contract_only = Registry::default();
    register_learned_contracts(&mut contract_only);
    assert_eq!(
        contract_only.node_availability("learned/infer").state,
        AvailabilityState::ContractOnly
    );
    assert_eq!(
        contract_only.resolve(&panel).unwrap_err().code,
        "CND-IMP-001"
    );

    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let mut stale = installed.input.clone();
    let candidate = stale
        .candidates
        .iter_mut()
        .find(|candidate| candidate.implementation.semantic_contract.id == "learned/infer")
        .unwrap();
    candidate.host_report.valid_until_tick = stale.current_tick - 1;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(source, &stale).unwrap_err().code(),
        "CND-CMP-006"
    );
}

#[test]
fn inference_schema_runtime_device_resource_and_bounds_fail_closed() {
    for (from, to, code) in [
        (
            "conduit-fixed-linear",
            "unsupported-format",
            "CND-LEARN-002",
        ),
        (
            "conduit.learned/runtime/rust-fixed-linear",
            "conduit.learned/runtime/missing",
            "CND-LEARN-006",
        ),
        ("opset = 0", "opset = 1", "CND-LEARN-002"),
        (
            "conduit.learned/device/cpu-reference",
            "conduit.learned/device/missing",
            "CND-LEARN-006",
        ),
        (
            "conduit.learned/resource/cpu-reference-0",
            "conduit.learned/resource/cpu-reference-1",
            "CND-LEARN-006",
        ),
        (
            "input_shape = \"1x4\"",
            "input_shape = \"1x5\"",
            "CND-LEARN-006",
        ),
        ("maximum_batch = 1", "maximum_batch = 2", "CND-LEARN-006"),
        (
            "sensitivity = \"public\"",
            "sensitivity = \"secret\"",
            "CND-LEARN-003",
        ),
    ] {
        let source =
            include_str!("../../../examples/learned-fixed-inference.panel").replacen(from, to, 1);
        let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "mutation {from} unexpectedly ran");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn learned_values_remain_domain_specific_and_training_free() {
    let mut registry = Registry::default();
    register_learned_contracts(&mut registry);
    let ids = registry
        .contracts()
        .map(|contract| contract.id.as_str())
        .collect::<Vec<_>>();
    assert!(ids.contains(&"learned/model/literal"));
    assert!(ids.contains(&"learned/infer"));
    assert!(
        !ids.iter()
            .any(|id| id.contains("train") || id.contains("promotion") || id.contains("job"))
    );
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("speech/") || id.starts_with("robotics/"))
    );
}

#[test]
fn exact_lifecycle_plan_pins_training_evaluation_and_promotion_authority() {
    let source = include_str!("../../../examples/learned-lifecycle.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_lifecycle_fixture_provider(&mut registry).unwrap();
    let constraints = observed_host_service_constraints(&PROMOTION_AUTHORITY_CONSTRAINTS);
    let authority = fixture_host_service_authority_observation(
        "learned/promote",
        "root/promote",
        "conduit/conduct-run",
        1,
        &constraints,
    )
    .unwrap();
    let installed =
        InstalledProfile::observe_registry_with_host_authorities(source, &registry, &[authority])
            .unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    let promotion = document
        .nodes
        .iter()
        .find(|node| node.contract.id == "learned/promote")
        .unwrap();
    assert_eq!(
        promotion.implementation.id,
        "conduit.learned/promote-fixture"
    );
    assert_eq!(document.authorities.len(), 1);
    assert_eq!(
        document.authorities[0].effect.action,
        "conduit.action/promote"
    );

    for (path, expected) in [
        (
            "examples/learned-lifecycle-standalone.panel",
            "learned:dataset:tiny:train:4:public",
        ),
        (
            "examples/learned-evaluation.panel",
            "learned:evaluation:accuracy@1:4/4:not-approval",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .arg(workspace_file(path))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_conduct"))
        .arg(workspace_file("examples/learned-lifecycle.panel"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("CND-IMP-001"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn promotion_effect_failures_run_through_the_fixture_backend_without_receipts() {
    for (fault, code) in [
        (PromotionFixtureFault::Rejected, "CND-LEARN-019"),
        (PromotionFixtureFault::ProviderLost, "CND-LEARN-016"),
        (
            PromotionFixtureFault::AfterCommitBeforeAcknowledgement,
            "CND-LEARN-020",
        ),
        (PromotionFixtureFault::Duplicate, "CND-LEARN-020"),
        (
            PromotionFixtureFault::InexactAcknowledgement,
            "CND-LEARN-020",
        ),
        (PromotionFixtureFault::BeforeCommit, "CND-LEARN-019"),
    ] {
        let error = execute_promotion_fixture(
            fault,
            PromotionUseFault::None,
            "conduit/conduct-run",
            1,
            "conduit/conduct-run",
            1,
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, code, "{fault:?}: {error}");
    }
}

#[test]
fn promotion_use_time_authority_run_epoch_and_cancellation_fail_closed() {
    for (use_fault, planned_run, planned_epoch, actual_run, actual_epoch, code) in [
        (
            PromotionUseFault::None,
            "",
            1,
            "conduit/conduct-run",
            1,
            "CND-LEARN-019",
        ),
        (
            PromotionUseFault::Revoked,
            "conduit/conduct-run",
            1,
            "conduit/conduct-run",
            1,
            "CND-RUN-010",
        ),
        (
            PromotionUseFault::WrongResource,
            "conduit/conduct-run",
            1,
            "conduit/conduct-run",
            1,
            "CND-RUN-010",
        ),
        (
            PromotionUseFault::WrongResourceGeneration,
            "conduit/conduct-run",
            1,
            "conduit/conduct-run",
            1,
            "CND-RUN-007",
        ),
        (
            PromotionUseFault::None,
            "conduit/wrong-run",
            1,
            "conduit/conduct-run",
            1,
            "CND-LEARN-019",
        ),
        (
            PromotionUseFault::None,
            "conduit/conduct-run",
            2,
            "conduit/conduct-run",
            1,
            "CND-LEARN-019",
        ),
    ] {
        let error = execute_promotion_fixture(
            PromotionFixtureFault::None,
            use_fault,
            planned_run,
            planned_epoch,
            actual_run,
            actual_epoch,
            false,
        )
        .unwrap_err();
        assert_eq!(error.code, code, "{error}");
    }

    let (report, display) = execute_promotion_fixture(
        PromotionFixtureFault::None,
        PromotionUseFault::None,
        "conduit/conduct-run",
        1,
        "conduit/conduct-run",
        1,
        true,
    )
    .unwrap();
    assert_eq!(report.summary.nodes_completed, 0);
    assert!(
        display.is_empty(),
        "cancelled run emitted a promotion result"
    );
}

#[test]
fn stale_promotion_provider_observation_is_rejected_before_execution() {
    let source = include_str!("../../../examples/learned-lifecycle.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_lifecycle_fixture_provider(&mut registry).unwrap();
    let installed = InstalledProfile::observe_registry_with_host_authorities(
        source,
        &registry,
        &[promotion_observation("conduit/conduct-run", 1)],
    )
    .unwrap();
    let mut stale = installed.input.clone();
    let candidate = stale
        .candidates
        .iter_mut()
        .find(|candidate| candidate.implementation.semantic_contract.id == "learned/promote")
        .unwrap();
    candidate.host_report.valid_until_tick = stale.current_tick;
    stale.seal().unwrap();
    assert_eq!(
        compile_source(source, &stale).unwrap_err().code(),
        "CND-CMP-008"
    );
}

#[test]
fn training_and_evaluation_are_runnable_without_a_promotion_provider_or_grant() {
    let source = include_str!("../../../examples/learned-evaluation.panel");
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_training_provider(&mut registry).unwrap();
    assert_eq!(
        registry.node_availability("learned/promote").state,
        AvailabilityState::ContractOnly
    );
    let installed = InstalledProfile::observe_registry(source, &registry).unwrap();
    let document = compile_source(source, &installed.input).unwrap();
    assert!(document.authorities.is_empty());
}

#[test]
fn learned_lifecycle_schema_resources_metrics_and_promotion_fail_closed() {
    for (from, to, code) in [
        (
            "sensitivity = \"public\"",
            "sensitivity = \"secret\"",
            "CND-LEARN-012",
        ),
        (
            "sha256:d5ac227d73ef18638d38b51c67b816148cd18c837680cc2fb827e4ef773c5145",
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "CND-LEARN-011",
        ),
        ("maximum_work = 64", "maximum_work = 65", "CND-LEARN-013"),
        ("metric_version = 1", "metric_version = 2", "CND-LEARN-018"),
    ] {
        let source =
            include_str!("../../../examples/learned-evaluation.panel").replacen(from, to, 1);
        let mut child = Command::new(env!("CARGO_BIN_EXE_conduct"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(source.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(!output.status.success(), "mutation {from} unexpectedly ran");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let source = include_str!("../../../examples/learned-lifecycle.panel").replacen(
        "target_slot = \"learned/reference\"",
        "target_slot = \"learned/unapproved\"",
        1,
    );
    let mut registry = Registry::hosted_primitives();
    register_deterministic_inference_provider(&mut registry).unwrap();
    register_deterministic_lifecycle_fixture_provider(&mut registry).unwrap();
    let error = InstalledProfile::observe_registry_with_host_authorities(
        &source,
        &registry,
        &[promotion_observation("conduit/conduct-run", 1)],
    )
    .err()
    .expect("unapproved target was rejected");
    assert_eq!(error.code, "CND-LEARN-019");
}
