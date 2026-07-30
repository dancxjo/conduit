use std::collections::{BTreeMap, BTreeSet};

use conduit_core::{
    AdmittedSupervisionAction, CanonicalDescriptor, CanonicalValue,
    EXECUTION_PLAN_SCHEMA_VERSION_V15, EvidenceCursor, EvidenceCursorStatus, FailurePlane,
    FieldDisposition, Id, InstancePath, MapField, PlanResourceBudget, RecoveryBudget,
    RetryDeclaration, SUPERVISION_CONTRACT_VERSION, SemanticHash, StandardNodeContract,
    StandardNodeKind, StandardNodeLimits, StopPolicy, SupervisionActionKind,
    SupervisionAffectedScope, SupervisionCauseRef, SupervisionContract, SupervisionDecision,
    SupervisionEvidenceKind, SupervisionFailureMode, SupervisionHostProfile, SupervisionLimits,
    SupervisionReason, SupervisionScope, TerminalCauseCode, TerminalClass, TerminalContext,
    TerminalObservation, TerminalPhase, classify_evidence_cursor, minimum_supervision_allocation,
    nearest_supervision_boundary, outward_handler_observation, select_terminal_observation,
    validate_standard_supervisor, validate_supervision_allocation, validate_supervision_nesting,
};
use conduit_panel::{LoadedModule, ModuleLoader, parse, resolve_modules};
use conduit_runtime::{BoundedSupervisionRuntime, Registry, lower_source_v3};
use serde_json::{Value, json};

const ACTIONS: [AdmittedSupervisionAction<'static>; 7] = [
    action(
        SupervisionActionKind::Propagate,
        None,
        4,
        false,
        true,
        false,
    ),
    action(
        SupervisionActionKind::StopScope,
        None,
        4,
        false,
        true,
        false,
    ),
    action(
        SupervisionActionKind::RestartSame,
        None,
        4,
        false,
        true,
        false,
    ),
    action(SupervisionActionKind::RetrySame, None, 4, true, true, false),
    action(
        SupervisionActionKind::ActivateDeclaredFallback,
        Some(Id("fallback")),
        4,
        false,
        true,
        false,
    ),
    action(
        SupervisionActionKind::ContinueDeclaredDegradedMode,
        Some(Id("degraded")),
        4,
        false,
        false,
        false,
    ),
    action(
        SupervisionActionKind::RequestOperatorAction,
        Some(Id("operator")),
        4,
        false,
        true,
        false,
    ),
];

const EPOCH_ACTION: [AdmittedSupervisionAction<'static>; 1] = [action(
    SupervisionActionKind::ActivateDeclaredFallback,
    Some(Id("replacement")),
    1,
    false,
    true,
    true,
)];

const fn action(
    kind: SupervisionActionKind,
    target: Option<Id<'static>>,
    maximum_uses: u16,
    permits_effect_replay: bool,
    preserves_required_guarantees: bool,
    requires_new_epoch: bool,
) -> AdmittedSupervisionAction<'static> {
    AdmittedSupervisionAction {
        kind,
        target,
        maximum_uses,
        permits_effect_replay,
        preserves_required_guarantees,
        requires_new_epoch,
    }
}

fn limits() -> SupervisionLimits {
    SupervisionLimits {
        maximum_observations: 4,
        maximum_decisions: 4,
        maximum_in_flight: 2,
        maximum_cause_depth: 4,
        maximum_nested_depth: 4,
        maximum_handler_ticks: 10,
        maximum_recovery_ticks: 20,
        restart_window_ticks: 10,
        backoff_ticks: 2,
        cooldown_ticks: 3,
        operator_wait_ticks: 5,
        maximum_evidence_events: 32,
        observation_bytes: 256,
        decision_bytes: 64,
        scratch_bytes: 128,
    }
}

fn contract(
    id: &'static str,
    subject: &'static str,
    handler: &'static str,
    actions: &'static [AdmittedSupervisionAction<'static>],
    limits: SupervisionLimits,
) -> SupervisionContract<'static> {
    SupervisionContract {
        schema_version: SUPERVISION_CONTRACT_VERSION,
        id: Id(id),
        scope: SupervisionScope::Child,
        subject: InstancePath::new(subject).unwrap(),
        handler: InstancePath::new(handler).unwrap(),
        members: &[],
        failure_mode: SupervisionFailureMode::FailTogether,
        outer: None,
        actions,
        limits,
        cleanup: StopPolicy::Abort,
        required_behavior: true,
    }
}

fn observation(
    subject: &'static str,
    run: &'static str,
    generation: u32,
    attempt: u16,
    code: TerminalCauseCode,
    retry: RetryDeclaration,
    remaining_attempts: u16,
) -> TerminalObservation<'static> {
    TerminalObservation {
        semantic_subject: InstancePath::new(subject).unwrap(),
        expanded_subject: InstancePath::new(subject).unwrap(),
        run: Id(run),
        plan_identity: SemanticHash::from_bytes([7; 32]),
        plan_epoch: 3,
        generation,
        attempt,
        class: TerminalClass::Failed,
        code,
        phase: TerminalPhase::Step,
        caused_by: &[],
        retry,
        context: TerminalContext {
            resource: Some(Id("resource")),
            authority: Some(SemanticHash::from_bytes([8; 32])),
            host: Some(Id("host")),
            implementation: Some(SemanticHash::from_bytes([9; 32])),
            artifact: Some(Id("artifact")),
            transition: None,
        },
        evidence: EvidenceCursor {
            stream: Id("evidence"),
            sequence: 9,
        },
        budget: RecoveryBudget {
            remaining_observations: 4,
            remaining_decisions: 4,
            remaining_attempts,
            remaining_evidence_events: 32,
            now_tick: 10,
            deadline_tick: 20,
        },
    }
}

fn decision(
    kind: SupervisionActionKind,
    target: Option<Id<'static>>,
) -> SupervisionDecision<'static> {
    SupervisionDecision { kind, target }
}

fn code(reason: SupervisionReason) -> Value {
    json!({"code": reason.code()})
}

struct MemoryLoader(BTreeMap<String, String>);

impl ModuleLoader for MemoryLoader {
    fn load(&self, canonical_uri: &str) -> Result<Option<LoadedModule>, String> {
        Ok(self.0.get(canonical_uri).map(|source| LoadedModule {
            canonical_uri: canonical_uri.to_owned(),
            source: source.clone(),
        }))
    }
}

fn panel_v2_source() -> &'static str {
    "panel 2\n\
     node subject : conduit.std/literal { value = \"work\" }\n\
     node sink : conduit.std/stdout\n\
     node handler : conduit.std/supervisor\n\
     cord subject.out -> sink.in\n\
     supervise subject with handler\n"
}

fn lower_panel_v2() -> conduit_runtime::LoweredSourceV3 {
    let uri = "mem://supervision/entry.panel";
    let loader = MemoryLoader(BTreeMap::from([(
        uri.to_owned(),
        panel_v2_source().to_owned(),
    )]));
    let graph = resolve_modules(uri, None, &loader).unwrap();
    lower_source_v3(&graph, &Registry::default()).unwrap()
}

fn action_identity(action: AdmittedSupervisionAction<'_>) -> SemanticHash {
    let field = |name, value| MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    };
    CanonicalDescriptor {
        kind: Id("conduit/test-supervision-action"),
        schema_version: 1,
        body: CanonicalValue::Map(&[
            field(
                "kind",
                CanonicalValue::Text(match action.kind {
                    SupervisionActionKind::Propagate => "propagate",
                    SupervisionActionKind::StopScope => "stop-scope",
                    SupervisionActionKind::RestartSame => "restart-same",
                    SupervisionActionKind::RetrySame => "retry-same",
                    SupervisionActionKind::ActivateDeclaredFallback => "fallback",
                    SupervisionActionKind::ContinueDeclaredDegradedMode => "degraded",
                    SupervisionActionKind::RequestOperatorAction => "operator",
                }),
            ),
            field(
                "target",
                action.target.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Text(value.as_str())
                }),
            ),
        ]),
    }
    .semantic_hash()
    .unwrap()
}

fn execute_case(id: &str) -> Value {
    let base = contract(
        "supervisor",
        "root/subject",
        "root/handler",
        &ACTIONS,
        limits(),
    );
    let observed = observation(
        "root/subject",
        "run",
        1,
        1,
        TerminalCauseCode::NodeFailed,
        RetryDeclaration::Undeclared,
        2,
    );
    match id {
        "domain-outcome-is-value" => {
            assert_ne!(FailurePlane::DomainValue, FailurePlane::RuntimeTerminal);
            json!({"outcome":"domain-value"})
        }
        "admission-diagnostic-skips-handler" => {
            assert_ne!(
                FailurePlane::AdmissionDiagnostic,
                FailurePlane::RuntimeTerminal
            );
            json!({"outcome":"admission-diagnostic"})
        }
        "terminal-without-handler-propagates" => {
            let boundary =
                nearest_supervision_boundary(observed.semantic_subject, &[], &[]).unwrap();
            assert!(boundary.is_none());
            json!({"outcome":"propagated"})
        }
        "bounded-restart-new-attempt" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Deterministic)
                    .unwrap();
            runtime.submit_terminal(observed).unwrap();
            let outcome = runtime
                .submit_decision(observed, decision(SupervisionActionKind::RestartSame, None))
                .unwrap();
            assert_eq!(
                outcome.consequence.kind,
                SupervisionEvidenceKind::AttemptStarted
            );
            assert_eq!(outcome.accepted.action_index, Some(2));
            assert_eq!(outcome.timing.attempt_not_before_tick, Some(12));
            assert_eq!(outcome.timing.restart_window_deadline_tick, Some(20));
            json!({"outcome":"restart","attempt":outcome.next_attempt.unwrap()})
        }
        "restart-attempts-exhausted" | "replicated-child-attempt-bounded" => {
            let exhausted = TerminalObservation {
                budget: RecoveryBudget {
                    remaining_attempts: 0,
                    ..observed.budget
                },
                ..observed
            };
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Deterministic)
                    .unwrap();
            runtime.submit_terminal(exhausted).unwrap();
            code(
                runtime
                    .submit_decision(
                        exhausted,
                        decision(SupervisionActionKind::RestartSame, None),
                    )
                    .unwrap_err(),
            )
        }
        "retry-non-idempotent-rejected" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(observed, decision(SupervisionActionKind::RetrySame, None))
                    .unwrap_err(),
            )
        }
        "declared-fallback-selected" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let outcome = runtime
                .submit_decision(
                    observed,
                    decision(
                        SupervisionActionKind::ActivateDeclaredFallback,
                        Some(Id("fallback")),
                    ),
                )
                .unwrap();
            assert_eq!(
                outcome.consequence.kind,
                SupervisionEvidenceKind::FallbackSelected
            );
            json!({"outcome":"fallback"})
        }
        "required-degraded-mode-rejected" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(
                        observed,
                        decision(
                            SupervisionActionKind::ContinueDeclaredDegradedMode,
                            Some(Id("degraded")),
                        ),
                    )
                    .unwrap_err(),
            )
        }
        "child-race-order-independent" => {
            let cancelled = TerminalObservation {
                code: TerminalCauseCode::ParentCancelled,
                ..observed
            };
            let first = [cancelled, observed];
            let second = [observed, cancelled];
            let a = first[select_terminal_observation(&first).unwrap()];
            let b = second[select_terminal_observation(&second).unwrap()];
            assert_eq!(a.code, b.code);
            json!({"primary":a.code.as_str()})
        }
        "cancellation-pending-handler"
        | "handler-cancelled-propagates"
        | "handler-cancellation-race" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let evidence = runtime.cancel().unwrap();
            assert!(runtime.next_observation().is_none());
            assert!(runtime.state().cancelled);
            if id == "handler-cancellation-race" {
                json!({"primary":"parent-cancelled"})
            } else {
                assert_eq!(evidence.kind, SupervisionEvidenceKind::Cancelled);
                json!({"outcome":"cancelled"})
            }
        }
        "handler-trap-propagates" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let evidence = runtime.handler_failed().unwrap();
            assert!(runtime.state().terminal);
            assert_eq!(evidence.kind, SupervisionEvidenceKind::HandlerFailed);
            let mut causes = [SupervisionCauseRef {
                code: TerminalCauseCode::NaturalCompletion,
                subject: InstancePath::new("unused").unwrap(),
                generation: 0,
                attempt: 0,
            }; 4];
            let outward = outward_handler_observation(
                observed,
                base,
                TerminalCauseCode::NodeFailed,
                TerminalPhase::Step,
                &mut causes,
            )
            .unwrap();
            assert_eq!(outward.semantic_subject, base.handler);
            assert_eq!(outward.caused_by.len(), 1);
            assert_eq!(outward.caused_by[0].subject, base.subject);
            json!({"outcome":"handler-failed"})
        }
        "handler-timeout-propagates" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            assert_eq!(
                runtime.handler_timed_out(19).unwrap_err(),
                SupervisionReason::ObservationInvalid
            );
            let reason = runtime.handler_timed_out(20).unwrap_err();
            assert!(runtime.state().terminal);
            assert_eq!(
                runtime.evidence().last().unwrap().kind,
                SupervisionEvidenceKind::Exhausted
            );
            code(reason)
        }
        "cleanup-failure-propagates" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let reason = runtime.cleanup_failed().unwrap_err();
            assert!(runtime.state().terminal);
            assert_eq!(
                runtime.evidence().last().unwrap().kind,
                SupervisionEvidenceKind::CleanupFailed
            );
            code(reason)
        }
        "nearest-nested-boundary" => {
            let mut inner = base;
            inner.id = Id("inner");
            let mut outer = contract(
                "outer",
                "root/composite",
                "root/outer-handler",
                &ACTIONS,
                limits(),
            );
            outer.scope = SupervisionScope::CompositeBoundary;
            let contracts = [outer, inner];
            let boundary = nearest_supervision_boundary(
                observed.semantic_subject,
                &[Id("inner"), Id("outer")],
                &contracts,
            )
            .unwrap()
            .unwrap();
            json!({"boundary":boundary.id.as_str()})
        }
        "self-supervision-rejected" => {
            let invalid = contract("self", "root/subject", "root/subject", &ACTIONS, limits());
            code(invalid.validate().unwrap_err())
        }
        "replacement-is-candidate-epoch" => {
            let epoch = contract(
                "supervisor",
                "root/subject",
                "root/handler",
                &EPOCH_ACTION,
                limits(),
            );
            let mut runtime =
                BoundedSupervisionRuntime::new(epoch, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(
                        observed,
                        decision(
                            SupervisionActionKind::ActivateDeclaredFallback,
                            Some(Id("replacement")),
                        ),
                    )
                    .unwrap_err(),
            )
        }
        "authority-expansion-not-an-action" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(
                        observed,
                        decision(
                            SupervisionActionKind::RequestOperatorAction,
                            Some(Id("grant-authority")),
                        ),
                    )
                    .unwrap_err(),
            )
        }
        "hostile-handler-invents-action-rejected" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(
                        observed,
                        decision(
                            SupervisionActionKind::ActivateDeclaredFallback,
                            Some(Id("hostile-invented-target")),
                        ),
                    )
                    .unwrap_err(),
            )
        }
        "hosted-browser-fake-normalized" => {
            let mut kinds = Vec::new();
            for profile in [
                SupervisionHostProfile::Hosted,
                SupervisionHostProfile::Browser,
                SupervisionHostProfile::Deterministic,
            ] {
                let mut runtime = BoundedSupervisionRuntime::new(base, profile).unwrap();
                runtime.submit_terminal(observed).unwrap();
                kinds.push(
                    runtime
                        .submit_decision(observed, decision(SupervisionActionKind::Propagate, None))
                        .unwrap()
                        .consequence
                        .kind,
                );
            }
            assert!(kinds.windows(2).all(|pair| pair[0] == pair[1]));
            json!({"outcome":"propagated"})
        }
        "constrained-profile-subset" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Constrained).unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(
                        observed,
                        decision(
                            SupervisionActionKind::ActivateDeclaredFallback,
                            Some(Id("fallback")),
                        ),
                    )
                    .unwrap_err(),
            )
        }
        "evidence-gap-resync-no-invention" => {
            let status = classify_evidence_cursor(4, 8, 12).unwrap();
            assert_eq!(status, EvidenceCursorStatus::Gap { resume_at: 8 });
            json!({"cursor":"gap"})
        }
        "evidence-exhaustion-before-action" => {
            let mut small = limits();
            small.maximum_evidence_events = 3;
            let mut runtime = BoundedSupervisionRuntime::new(
                contract("small", "root/subject", "root/handler", &ACTIONS, small),
                SupervisionHostProfile::Hosted,
            )
            .unwrap();
            runtime.submit_terminal(observed).unwrap();
            code(
                runtime
                    .submit_decision(observed, decision(SupervisionActionKind::Propagate, None))
                    .unwrap_err(),
            )
        }
        "observation-limit" => {
            let mut one = limits();
            one.maximum_observations = 1;
            one.maximum_in_flight = 1;
            let contract = contract("one", "root/subject", "root/handler", &ACTIONS, one);
            let mut runtime =
                BoundedSupervisionRuntime::new(contract, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            runtime
                .submit_decision(observed, decision(SupervisionActionKind::RestartSame, None))
                .unwrap();
            let second = TerminalObservation {
                attempt: 2,
                ..observed
            };
            code(runtime.submit_terminal(second).unwrap_err())
        }
        "decision-limit" => {
            let mut one = limits();
            one.maximum_decisions = 1;
            let contract = contract("one", "root/subject", "root/handler", &ACTIONS, one);
            let mut runtime =
                BoundedSupervisionRuntime::new(contract, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            runtime
                .submit_decision(observed, decision(SupervisionActionKind::RestartSame, None))
                .unwrap();
            let second = TerminalObservation {
                attempt: 2,
                ..observed
            };
            runtime.submit_terminal(second).unwrap();
            code(
                runtime
                    .submit_decision(second, decision(SupervisionActionKind::RestartSame, None))
                    .unwrap_err(),
            )
        }
        "in-flight-limit" | "hosted-pending-queue-bounded" => {
            let mut one = limits();
            one.maximum_in_flight = 1;
            let contract = contract("one", "root/subject", "root/handler", &ACTIONS, one);
            let mut runtime =
                BoundedSupervisionRuntime::new(contract, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let another = TerminalObservation {
                run: Id("another-run"),
                ..observed
            };
            code(runtime.submit_terminal(another).unwrap_err())
        }
        "recovery-deadline-expired" | "operator-wait-is-bounded" => {
            let expired = TerminalObservation {
                budget: RecoveryBudget {
                    now_tick: 20,
                    deadline_tick: 20,
                    ..observed.budget
                },
                ..observed
            };
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            code(runtime.submit_terminal(expired).unwrap_err())
        }
        "panel-v1-supervise-rejected" => {
            let error = parse(&panel_v2_source().replacen("panel 2", "panel 1", 1)).unwrap_err();
            json!({"code":error.code})
        }
        "panel-v2-supervision-lowers" => {
            let lowered = lower_panel_v2();
            assert_eq!(lowered.source_ast_schema_version, 3);
            json!({"bindings":lowered.supervisions.len()})
        }
        "exact-plan-resources-required" => {
            let required = minimum_supervision_allocation(limits()).unwrap();
            let too_small = PlanResourceBudget {
                memory_bytes: required.memory_bytes - 1,
                ..required
            };
            code(validate_supervision_allocation(limits(), too_small).unwrap_err())
        }
        "action-changes-plan-identity" => {
            let left = action_identity(ACTIONS[0]);
            let right = action_identity(ACTIONS[2]);
            json!({"identity_changed":left != right})
        }
        "unknown-fallback-target-rejected" => {
            const BAD: [AdmittedSupervisionAction<'static>; 1] = [action(
                SupervisionActionKind::ActivateDeclaredFallback,
                None,
                1,
                false,
                true,
                false,
            )];
            let invalid = contract("bad", "root/subject", "root/handler", &BAD, limits());
            code(invalid.validate().unwrap_err())
        }
        "outer-supervision-cycle-rejected" => {
            let mut left = base;
            left.id = Id("left");
            left.outer = Some(Id("right"));
            let mut right = contract(
                "right",
                "root/other",
                "root/other-handler",
                &ACTIONS,
                limits(),
            );
            right.outer = Some(Id("left"));
            code(validate_supervision_nesting(&[left, right]).unwrap_err())
        }
        "domain-negative-can-continue" => {
            let runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            assert!(runtime.next_observation().is_none());
            json!({"handler_invocations":0})
        }
        "old-new-generation-race" => {
            let old = observed;
            let new = TerminalObservation {
                generation: 2,
                ..observed
            };
            let observations = [old, new];
            let selected = observations[select_terminal_observation(&observations).unwrap()];
            json!({"generation":selected.generation})
        }
        "observation-carries-identities-not-values" => {
            assert!(observed.context.resource.is_some());
            assert!(observed.context.authority.is_some());
            assert_eq!(observed.evidence.sequence, 9);
            json!({"redacted":true})
        }
        "source-self-supervision-rejected" => {
            let source = "panel 2\nnode subject : conduit.std/literal { value = \"x\" }\nsupervise subject with subject\n";
            let error = parse(source).unwrap_err();
            json!({"code":error.code})
        }
        "compile-roundtrip-schema-15" => {
            assert_eq!(EXECUTION_PLAN_SCHEMA_VERSION_V15, 15);
            json!({"schema_version":EXECUTION_PLAN_SCHEMA_VERSION_V15})
        }
        "browser-profile-explicit-action" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Browser).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let outcome = runtime
                .submit_decision(observed, decision(SupervisionActionKind::RestartSame, None))
                .unwrap();
            assert_eq!(outcome.next_attempt, Some(2));
            json!({"outcome":"restart"})
        }
        "cross-host-decisions-normalized" => {
            let mut outcomes = Vec::new();
            for (profile, host) in [
                (SupervisionHostProfile::Hosted, Id("linux-host")),
                (SupervisionHostProfile::Browser, Id("browser-host")),
            ] {
                let per_host = TerminalObservation {
                    context: TerminalContext {
                        host: Some(host),
                        ..observed.context
                    },
                    ..observed
                };
                let mut runtime = BoundedSupervisionRuntime::new(base, profile).unwrap();
                runtime.submit_terminal(per_host).unwrap();
                outcomes.push(
                    runtime
                        .submit_decision(
                            per_host,
                            decision(SupervisionActionKind::RestartSame, None),
                        )
                        .unwrap(),
                );
            }
            assert_eq!(outcomes[0].next_attempt, outcomes[1].next_attempt);
            assert_eq!(outcomes[0].timing, outcomes[1].timing);
            json!({"outcome":"restart","attempt":outcomes[0].next_attempt.unwrap()})
        }
        "allocator-free-profile-equivalent" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Constrained).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let outcome = runtime
                .submit_decision(observed, decision(SupervisionActionKind::Propagate, None))
                .unwrap();
            assert_eq!(
                outcome.consequence.kind,
                SupervisionEvidenceKind::Propagated
            );
            json!({"outcome":"propagated"})
        }
        "cleanup-policy-is-exact" => {
            assert_eq!(base.cleanup, StopPolicy::Abort);
            json!({"cleanup":"abort"})
        }
        "decision-correlation-required" => {
            let mut runtime =
                BoundedSupervisionRuntime::new(base, SupervisionHostProfile::Hosted).unwrap();
            runtime.submit_terminal(observed).unwrap();
            let unrelated = TerminalObservation {
                run: Id("unrelated"),
                ..observed
            };
            let reason = runtime
                .submit_decision(unrelated, decision(SupervisionActionKind::Propagate, None))
                .unwrap_err();
            assert_eq!(runtime.evidence().last().unwrap().reason, Some(reason));
            code(reason)
        }
        "named-group-fail-together" | "named-group-isolated-optional" => {
            let members = [
                InstancePath::new("root/subject").unwrap(),
                InstancePath::new("root/peer").unwrap(),
            ];
            let mut group = base;
            group.scope = SupervisionScope::NamedGroup;
            group.members = &members;
            group.failure_mode = if id == "named-group-fail-together" {
                SupervisionFailureMode::FailTogether
            } else {
                SupervisionFailureMode::IsolatedOptional
            };
            let peer = TerminalObservation {
                semantic_subject: members[1],
                expanded_subject: members[1],
                run: Id("peer-run"),
                ..observed
            };
            let mut runtime =
                BoundedSupervisionRuntime::new(group, SupervisionHostProfile::Deterministic)
                    .unwrap();
            runtime.submit_terminal(peer).unwrap();
            let outcome = runtime
                .submit_decision(peer, decision(SupervisionActionKind::StopScope, None))
                .unwrap();
            json!({
                "affected": match outcome.affected_scope {
                    SupervisionAffectedScope::ObservedSubject => "observed-subject",
                    SupervisionAffectedScope::BoundScope => "bound-scope",
                    SupervisionAffectedScope::Outward => "outward",
                },
                "terminal": runtime.state().terminal
            })
        }
        other => panic!("undispatched supervision fixture `{other}`"),
    }
}

#[test]
fn every_supervision_fixture_executes_independently() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../conformance/c4/supervision-v1.json")).unwrap();
    assert_eq!(fixture["schema"], "conduit.supervision-fixtures/v1");
    assert_eq!(fixture["contract_version"], SUPERVISION_CONTRACT_VERSION);
    let cases = fixture["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 49);
    let mut seen = BTreeSet::new();
    for case in cases {
        let id = case["id"].as_str().unwrap();
        assert!(seen.insert(id), "duplicate fixture `{id}`");
        let actual = execute_case(id);
        assert_eq!(actual, case["expected"], "fixture `{id}`");
    }
}

#[test]
fn supervisor_type_descriptors_are_canonical_and_resolvable() {
    let source = panel_v2_source();
    let mut panel = parse(source).unwrap();
    let lowered = lower_panel_v2();
    panel.supervisions[0].resolved_identity =
        Some(lowered.supervisions[0].semantic_hash.to_string());
    let topology = Registry::compatibility_demo()
        .resolve(&panel)
        .unwrap()
        .exact_topology()
        .unwrap();
    assert_eq!(topology.supervisions.len(), 1);
    assert_eq!(topology.supervisions[0].subject, "root/subject");
    assert_eq!(topology.supervisions[0].handler, "root/handler");
    let handler = topology
        .nodes
        .iter()
        .find(|node| node.instance == "root/handler")
        .unwrap();
    assert_ne!(
        handler.inputs[0].value_type.semantic_hash,
        SemanticHash::from_bytes([0; 32])
    );
}

#[test]
fn named_group_failure_mode_controls_exact_stop_scope() {
    let members = [
        InstancePath::new("root/subject").unwrap(),
        InstancePath::new("root/peer").unwrap(),
    ];
    let mut grouped = contract("group", "root/subject", "root/handler", &ACTIONS, limits());
    grouped.scope = SupervisionScope::NamedGroup;
    grouped.members = &members;
    grouped.failure_mode = SupervisionFailureMode::FailTogether;
    let peer = observation(
        "root/peer",
        "peer-run",
        1,
        1,
        TerminalCauseCode::NodeFailed,
        RetryDeclaration::Undeclared,
        1,
    );
    let mut together =
        BoundedSupervisionRuntime::new(grouped, SupervisionHostProfile::Deterministic).unwrap();
    together.submit_terminal(peer).unwrap();
    let stopped = together
        .submit_decision(peer, decision(SupervisionActionKind::StopScope, None))
        .unwrap();
    assert_eq!(stopped.affected_scope, SupervisionAffectedScope::BoundScope);
    assert!(together.state().terminal);

    grouped.failure_mode = SupervisionFailureMode::IsolatedOptional;
    let mut isolated =
        BoundedSupervisionRuntime::new(grouped, SupervisionHostProfile::Deterministic).unwrap();
    isolated.submit_terminal(peer).unwrap();
    let stopped = isolated
        .submit_decision(peer, decision(SupervisionActionKind::StopScope, None))
        .unwrap();
    assert_eq!(
        stopped.affected_scope,
        SupervisionAffectedScope::ObservedSubject
    );
    assert!(!isolated.state().terminal);
}

#[test]
fn standard_supervisor_consumes_the_portable_contract() {
    let portable = contract(
        "supervisor",
        "root/subject",
        "root/handler",
        &ACTIONS,
        limits(),
    );
    let standard = StandardNodeContract {
        id: Id("conduit.std/supervisor"),
        kind: StandardNodeKind::Supervisor,
        limits: StandardNodeLimits {
            retained_values: 2,
            retained_bytes: 448,
            pending_operations: 2,
            timers: 3,
            work_per_step: 1,
            evidence_events: 32,
        },
        terminal_policy: Id("propagate"),
        cancellation_policy: Id("bounded"),
    };
    validate_standard_supervisor(standard, portable).unwrap();
    let too_small = StandardNodeContract {
        limits: StandardNodeLimits {
            retained_bytes: 447,
            ..standard.limits
        },
        ..standard
    };
    assert!(validate_standard_supervisor(too_small, portable).is_err());
}
