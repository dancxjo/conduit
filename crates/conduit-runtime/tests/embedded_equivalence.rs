use std::cell::Cell;
use std::rc::Rc;

use conduit_core::{
    AuthorityTime, ExecutionPlan, ExecutionProfile, FlowEventKind, FlowQueueState, Id,
    ImplementationMachine, InstantiationContext, LifecycleUsage, PinnedDescriptor,
    PlanValidationContext, ReadyQueueDiscipline, ResolvedPlanNode, SCHEDULER_CONTRACT_VERSION,
    SchedulerPolicy, SemanticHash, StopPolicy,
};
use conduit_embedded::{
    EmbeddedEventKind, EmbeddedHostCall, EmbeddedHostServices, EmbeddedInterest, EmbeddedNode,
    EmbeddedOutcome, EmbeddedStep, EmbeddedStorage, EmbeddedValue, HostReply, InterestSet,
    RunControl, RunIdentity, RunStatus, StepContext, execute_static_plan,
};
use conduit_rp2040_hil::{
    FULL_PLAN_HASH as FIRMWARE_PLAN_HASH, GENERATED_EMBEDDED_PLAN_IDENTITY, GENERATED_NODES,
    plan as firmware_plan, profile as firmware_profile, reference_plan::with_equivalence_plans,
};
use conduit_runtime::{
    DeterministicExecutor, RuntimeValue, RuntimeValueEnvelope, ScheduledNode, SchedulerEventKind,
    SchedulerNode, SchedulerReservation, SchedulerStatus, SchedulerStep, SendStatus, StepIo,
};

const EQUIVALENCE_FIXTURE: &str = include_str!("../../../conformance/c5/embedded-equivalence.json");
fn machine<'a>(
    profile: &'a ExecutionProfile<'a>,
    node: &ResolvedPlanNode<'a>,
) -> ImplementationMachine {
    ImplementationMachine::instantiate(
        profile,
        InstantiationContext {
            instance: node.instance,
            implementation: node.implementation,
            artifact: node.artifact,
            execution_profile_hash: profile.semantic_hash,
            configuration_validated: true,
            caller_memory_bytes: 320,
            required_resource_bindings: &[],
            provided_resource_bindings: &[],
            required_grants: &[],
            provided_grants: &[],
            cancellation_scope: Id("scope/run"),
        },
    )
    .unwrap()
}

#[derive(Clone)]
enum DesktopDriver {
    Sensor { emitted: bool },
    Threshold,
    Indicator { value: Rc<Cell<Option<u64>>> },
}

impl SchedulerNode for DesktopDriver {
    fn prepare(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn start(&mut self) -> Result<LifecycleUsage, Id<'static>> {
        Ok(LifecycleUsage::default())
    }

    fn step(&mut self, io: &mut StepIo<'_>) -> SchedulerStep {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return SchedulerStep::Completed;
                }
                match io
                    .send(
                        0,
                        RuntimeValue {
                            handle: 42,
                            accounted_bytes: 4,
                            envelope: RuntimeValueEnvelope::EMPTY,
                        },
                        None,
                    )
                    .unwrap()
                {
                    SendStatus::Reserved => {
                        *emitted = true;
                        SchedulerStep::Progress
                    }
                    SendStatus::WouldBlock => {
                        io.wait_for_output(0).unwrap();
                        SchedulerStep::Pending
                    }
                    _ => SchedulerStep::Failed {
                        code: Id("fixture/send"),
                    },
                }
            }
            Self::Threshold => {
                if let Some(sample) = io.receive(0).unwrap() {
                    assert_eq!(sample.handle, 42);
                    match io
                        .send(
                            1,
                            RuntimeValue {
                                handle: u64::from(sample.handle >= 40),
                                accounted_bytes: 1,
                                envelope: RuntimeValueEnvelope::EMPTY,
                            },
                            None,
                        )
                        .unwrap()
                    {
                        SendStatus::Reserved => SchedulerStep::Progress,
                        SendStatus::WouldBlock => {
                            io.wait_for_output(1).unwrap();
                            SchedulerStep::Pending
                        }
                        _ => SchedulerStep::Failed {
                            code: Id("fixture/threshold"),
                        },
                    }
                } else if matches!(io.input_state(0).unwrap(), FlowQueueState::Completed) {
                    SchedulerStep::Completed
                } else {
                    io.wait_for_input(0).unwrap();
                    SchedulerStep::Pending
                }
            }
            Self::Indicator { value } => {
                if let Some(decision) = io.receive(1).unwrap() {
                    value.set(Some(decision.handle));
                    SchedulerStep::Progress
                } else if matches!(io.input_state(1).unwrap(), FlowQueueState::Completed) {
                    SchedulerStep::Completed
                } else {
                    io.wait_for_input(1).unwrap();
                    SchedulerStep::Pending
                }
            }
        }
    }
}

fn desktop_executor<'a>(
    plan: &'a ExecutionPlan<'a>,
    profile: &'a ExecutionProfile<'a>,
    indicator: Rc<Cell<Option<u64>>>,
) -> DeterministicExecutor<DesktopDriver> {
    DeterministicExecutor::start(
        plan,
        PlanValidationContext {
            supported_schema_version: 0,
            now: AuthorityTime {
                basis: Id("clock/monotonic"),
                tick: 2,
            },
        },
        SchedulerPolicy {
            schema_version: SCHEDULER_CONTRACT_VERSION,
            ready_queue: ReadyQueueDiscipline::RoundRobin,
            max_decisions: 64,
            max_tick: 128,
            max_consecutive_yields: 4,
            max_events: 128,
        },
        SchedulerReservation {
            available_runtime_memory_bytes: 32_000_000,
            executor_overhead_limit_bytes: 31_000_000,
        },
        vec![
            ScheduledNode {
                driver: DesktopDriver::Sensor { emitted: false },
                machine: machine(profile, &plan.nodes[0]),
            },
            ScheduledNode {
                driver: DesktopDriver::Threshold,
                machine: machine(profile, &plan.nodes[1]),
            },
            ScheduledNode {
                driver: DesktopDriver::Indicator { value: indicator },
                machine: machine(profile, &plan.nodes[2]),
            },
        ],
    )
    .unwrap()
}

#[derive(Debug, Eq, PartialEq)]
struct Normalized {
    prepared: usize,
    accepted: Vec<u64>,
    consumed: Vec<u64>,
    pressure_entered: usize,
    pressure_cleared: usize,
    completed: usize,
    succeeded: bool,
}

fn normalize_desktop(executor: &DeterministicExecutor<DesktopDriver>) -> Normalized {
    let mut normalized = Normalized {
        prepared: 0,
        accepted: Vec::new(),
        consumed: Vec::new(),
        pressure_entered: 0,
        pressure_cleared: 0,
        completed: 0,
        succeeded: false,
    };
    for event in executor.events() {
        match event.kind {
            SchedulerEventKind::NodePrepared => normalized.prepared += 1,
            SchedulerEventKind::ValueAccepted => {
                normalized.accepted.push(event.value_handle.unwrap())
            }
            SchedulerEventKind::ValueConsumed => {
                normalized.consumed.push(event.value_handle.unwrap())
            }
            SchedulerEventKind::Cord(FlowEventKind::PressureEntered) => {
                normalized.pressure_entered += 1
            }
            SchedulerEventKind::Cord(FlowEventKind::PressureCleared) => {
                normalized.pressure_cleared += 1
            }
            SchedulerEventKind::NodeOutcome {
                outcome: conduit_core::StepOutcomeKind::Completed,
            } => normalized.completed += 1,
            SchedulerEventKind::Terminal(conduit_core::TerminalClass::Succeeded) => {
                normalized.succeeded = true
            }
            _ => {}
        }
    }
    normalized
}

struct EmbeddedHost {
    indicator: Option<u64>,
}

impl EmbeddedHostServices<16> for EmbeddedHost {
    fn invoke(&mut self, call: EmbeddedHostCall<'_, 16>) -> HostReply<16> {
        match call.binding.operation.as_str() {
            "fixture/read-sample" => {
                HostReply::Completed(EmbeddedValue::from_slice(&42_u32.to_be_bytes()).unwrap())
            }
            "fixture/write-indicator" => {
                self.indicator = Some(u64::from(call.request.bytes[0]));
                HostReply::Completed(EmbeddedValue::EMPTY)
            }
            _ => HostReply::Failed(Id("fixture/host")),
        }
    }
}

enum EmbeddedDriver {
    Sensor { emitted: bool },
    Threshold,
    Indicator,
}

impl EmbeddedNode<EmbeddedHost, 16, 4, 4> for EmbeddedDriver {
    fn descriptor(&self) -> PinnedDescriptor<'static> {
        match self {
            Self::Sensor { .. } => GENERATED_NODES[0].driver,
            Self::Threshold => GENERATED_NODES[1].driver,
            Self::Indicator => GENERATED_NODES[2].driver,
        }
    }

    fn step(&mut self, context: &mut StepContext<'_, '_, EmbeddedHost, 16, 4>) -> EmbeddedStep<4> {
        match self {
            Self::Sensor { emitted } => {
                if *emitted {
                    return EmbeddedStep::completed();
                }
                let HostReply::Completed(sample) =
                    context.invoke_host(0, EmbeddedValue::EMPTY).unwrap()
                else {
                    return failed();
                };
                context.send(0, sample).unwrap();
                *emitted = true;
                EmbeddedStep::progress()
            }
            Self::Threshold => {
                if let Some(sample) = context.input(0) {
                    let sample = u32::from_be_bytes(sample.bytes[..4].try_into().unwrap());
                    context.consume(0).unwrap();
                    context
                        .send(
                            0,
                            EmbeddedValue::from_slice(&[u8::from(sample >= 40)]).unwrap(),
                        )
                        .unwrap();
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
            Self::Indicator => {
                if context.input(0).is_some() {
                    let value = context.consume(0).unwrap();
                    let _ = context.invoke_host(1, value).unwrap();
                    EmbeddedStep::progress()
                } else if context.input_closed(0) {
                    EmbeddedStep::completed()
                } else {
                    EmbeddedStep::pending(InterestSet::one(EmbeddedInterest::Input(0)))
                }
            }
        }
    }
}

fn failed() -> EmbeddedStep<4> {
    EmbeddedStep {
        outcome: EmbeddedOutcome::Failed(Id("fixture/embedded")),
        interests: InterestSet::EMPTY,
    }
}

fn normalize_embedded(events: &[conduit_embedded::EmbeddedEvent<16>]) -> Normalized {
    let mut normalized = Normalized {
        prepared: 0,
        accepted: Vec::new(),
        consumed: Vec::new(),
        pressure_entered: 0,
        pressure_cleared: 0,
        completed: 0,
        succeeded: false,
    };
    for event in events {
        match event.kind {
            EmbeddedEventKind::NodePrepared => normalized.prepared += 1,
            EmbeddedEventKind::ValueAccepted => {
                normalized.accepted.push(decode_value(event.value.unwrap()))
            }
            EmbeddedEventKind::ValueConsumed => {
                normalized.consumed.push(decode_value(event.value.unwrap()))
            }
            EmbeddedEventKind::PressureEntered => normalized.pressure_entered += 1,
            EmbeddedEventKind::PressureCleared => normalized.pressure_cleared += 1,
            EmbeddedEventKind::NodeCompleted => normalized.completed += 1,
            EmbeddedEventKind::RunSucceeded => normalized.succeeded = true,
            _ => {}
        }
    }
    normalized
}

fn decode_value(value: EmbeddedValue<16>) -> u64 {
    match value.length {
        1 => u64::from(value.bytes[0]),
        4 => u64::from(u32::from_be_bytes(value.bytes[..4].try_into().unwrap())),
        other => panic!("unexpected value width {other}"),
    }
}

fn equivalence_fixture_expected(id: &str) -> serde_json::Value {
    let fixture: serde_json::Value = serde_json::from_str(EQUIVALENCE_FIXTURE).unwrap();
    fixture["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing executed equivalence fixture case {id}"))["expected"]
        .clone()
}
#[test]
fn desktop_and_rp2040_execute_one_semantic_plan_with_normalized_equivalence() {
    let expected = equivalence_fixture_expected("same-plan-normalized-equivalence");
    with_equivalence_plans(|desktop_plan, rp2040_plan, profile| {
        let desktop_indicator = Rc::new(Cell::new(None));
        let mut desktop = desktop_executor(&desktop_plan, profile, desktop_indicator.clone());
        assert_eq!(
            desktop.run_until_stalled().unwrap(),
            SchedulerStatus::Succeeded
        );
        assert_eq!(desktop_indicator.get(), Some(1));

        let static_plan = firmware_plan();
        let embedded_profile = firmware_profile();
        assert!(
            static_plan
                .cords
                .iter()
                .all(|cord| cord.maximum_value_bytes == 8)
        );
        assert_eq!(
            static_plan.generated_plan_hash,
            GENERATED_EMBEDDED_PLAN_IDENTITY
        );
        assert_eq!(static_plan.full_plan_hash, rp2040_plan.identity);
        assert_eq!(embedded_profile.maximum_host_operations, 2);
        assert_eq!(static_plan.nodes[0].host_operations.len(), 1);
        assert!(static_plan.nodes[1].host_operations.is_empty());
        assert_eq!(static_plan.nodes[2].host_operations.len(), 1);
        for (node_index, ordinal) in [(0_usize, 0_u16), (2, 1)] {
            let generated = static_plan.nodes[node_index].host_operations[0];
            let authority = rp2040_plan
                .authorities
                .iter()
                .find(|authority| {
                    authority.node == rp2040_plan.nodes[node_index].instance
                        && authority.effect_hash == generated.effect_hash
                })
                .expect("generated operation names an exact plan authority");
            let resource = rp2040_plan
                .resources
                .iter()
                .find(|resource| {
                    resource.node == authority.node && resource.id == generated.resource_binding
                })
                .expect("generated operation names an exact plan resource");
            assert_eq!(generated.ordinal, ordinal);
            assert_eq!(generated.operation, authority.effect.action);
            assert_eq!(generated.resource, resource.resource);
            assert_eq!(generated.grant_hash, authority.grant_hash);
            assert_eq!(generated.capability_id, authority.capability.id);
            assert_eq!(generated.grant_id, authority.grant.id);
            assert_eq!(generated.host, authority.binding.host);
            assert_eq!(generated.check_at_use, authority.binding.check_at_use);
            assert_eq!(
                generated.resource_lease_hash,
                resource
                    .lease
                    .expect("ordinary embedded authority has a finite lease")
                    .semantic_hash()
                    .unwrap()
            );
            assert_eq!(
                generated.commit_profile_hash,
                authority
                    .commit_profile
                    .expect("ordinary embedded authority has commit semantics")
                    .semantic_hash()
                    .unwrap()
            );
        }
        assert_ne!(
            GENERATED_EMBEDDED_PLAN_IDENTITY,
            SemanticHash::from_bytes([0; 32])
        );
        let mut storage = EmbeddedStorage::<3, 2, 4, 2, 16, 64, 4, 4>::new();
        let mut embedded_host = EmbeddedHost { indicator: None };
        let summary = execute_static_plan(
            &static_plan,
            &embedded_profile,
            &mut storage,
            &mut [
                EmbeddedDriver::Sensor { emitted: false },
                EmbeddedDriver::Threshold,
                EmbeddedDriver::Indicator,
            ],
            &mut embedded_host,
            RunIdentity {
                boot_id: [5; 16],
                run_sequence: 1,
            },
            RunControl {
                maximum_decisions: 64,
                cancellation_at_decision: None,
                initial_tick: 0,
            },
        )
        .unwrap();
        assert_eq!(summary.status, RunStatus::Succeeded);
        assert_eq!(embedded_host.indicator, Some(1));
        let normalized_equal = normalize_desktop(&desktop) == normalize_embedded(storage.events());
        assert!(normalized_equal);
        assert!(
            storage
                .events()
                .iter()
                .all(|event| event.plan == rp2040_plan.identity)
        );
        assert_eq!(
            serde_json::json!({
                "same_source_semantic_hash": desktop_plan.source_semantic_hash == rp2040_plan.source_semantic_hash,
                "distinct_exact_execution_plan_hashes": desktop_plan.identity != rp2040_plan.identity,
                "firmware_bound_to_rp2040_plan_hash": rp2040_plan.identity == FIRMWARE_PLAN_HASH,
                "normalized_lifecycle_values_pressure_terminal_equal": normalized_equal
            }),
            expected
        );
    });
}

#[test]
fn desktop_and_rp2040_abort_cancellation_are_both_terminal() {
    let expected = equivalence_fixture_expected("same-plan-abort-cancellation-equivalence");
    with_equivalence_plans(|desktop_plan, rp2040_plan, profile| {
        let mut desktop = desktop_executor(&desktop_plan, profile, Rc::new(Cell::new(None)));
        desktop.cancel(StopPolicy::Abort).unwrap();
        assert_eq!(
            desktop.run_until_stalled().unwrap(),
            SchedulerStatus::Cancelled
        );
        assert!(desktop.events().any(|event| matches!(
            event.kind,
            SchedulerEventKind::CancellationRequested {
                stop: StopPolicy::Abort
            }
        )));
        assert!(desktop.events().any(|event| matches!(
            event.kind,
            SchedulerEventKind::Terminal(conduit_core::TerminalClass::Cancelled)
        )));

        let static_plan = firmware_plan();
        let embedded_profile = firmware_profile();
        assert_eq!(static_plan.full_plan_hash, rp2040_plan.identity);
        let mut storage = EmbeddedStorage::<3, 2, 4, 2, 16, 64, 4, 4>::new();
        let summary = execute_static_plan(
            &static_plan,
            &embedded_profile,
            &mut storage,
            &mut [
                EmbeddedDriver::Sensor { emitted: false },
                EmbeddedDriver::Threshold,
                EmbeddedDriver::Indicator,
            ],
            &mut EmbeddedHost { indicator: None },
            RunIdentity {
                boot_id: [5; 16],
                run_sequence: 2,
            },
            RunControl {
                maximum_decisions: 4,
                cancellation_at_decision: Some(0),
                initial_tick: 0,
            },
        )
        .unwrap();
        assert_eq!(summary.status, RunStatus::Cancelled);
        assert!(
            storage
                .events()
                .iter()
                .any(|event| { event.kind == EmbeddedEventKind::CancellationRequested })
        );
        assert!(
            storage
                .events()
                .iter()
                .any(|event| event.kind == EmbeddedEventKind::RunCancelled)
        );
        assert_eq!(
            serde_json::json!({
                "same_source_semantic_hash": desktop_plan.source_semantic_hash == rp2040_plan.source_semantic_hash,
                "distinct_exact_execution_plan_hashes": desktop_plan.identity != rp2040_plan.identity,
                "firmware_bound_to_rp2040_plan_hash": rp2040_plan.identity == FIRMWARE_PLAN_HASH,
                "desktop_status": "cancelled",
                "embedded_status": "cancelled"
            }),
            expected
        );
    });
}
