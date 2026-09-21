#![cfg(feature = "form-catalog")]

#[path = "common/hybrid_plan.rs"]
mod hybrid_plan;

use conduit_ai::{
    Chunk, ExtractedSourceValue, ExtractionLineage, FusionStrategy, HybridFusionPolicy,
    HybridRetrievalReceipt, MechanismScore, RetrievalMechanism, RetrievalStage, RetrieverIdentity,
    SourceRef, SourceSpan, SourceSpanUnit, StageCandidate,
};
use conduit_core::{
    bind_active_play, bind_sign, verify_plan, BoundedResourceRef, ConfigurationValue, KindId,
    ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepInputBytes, StepIo, StepOperation,
        StepOutcome,
    },
    CordEndpoint, CordId, Failure, FailureCode, FixedRoutes, HostedSignLog, HostedValueStore,
    KernelEvent, KernelEventKind, NodeId, PortId, RouteRange, RouteTarget, SignQuery, ValueRef,
    ValueStorage,
};

const FUSION_NODE: NodeId = NodeId(4);
const SINK_NODE: NodeId = NodeId(5);
const MAX_VALUE_BYTES: u32 = 8_192;
const POLICY_IDENTITY: &str = "fusion/reciprocal-rank@1";

#[derive(Clone)]
struct SourceOperation {
    value: ValueRef,
    emitted: bool,
}

#[derive(Clone)]
struct FusionOperation {
    policy: HybridFusionPolicy,
    stages: [Option<RetrievalStage<ExtractedSourceValue>>; 4],
    expected: Vec<u8>,
    output: ValueRef,
    emitted: bool,
}

#[derive(Clone)]
struct SinkOperation;

#[derive(Clone)]
enum TestOperation {
    Source(SourceOperation),
    Fusion(Box<FusionOperation>),
    Sink(SinkOperation),
}

impl StepOperation<4> for TestOperation {
    fn step(&mut self, io: &mut StepIo<4>, bytes: &StepInputBytes<'_, 4>) -> StepOutcome {
        match self {
            Self::Source(operation) => {
                if operation.emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), operation.value).unwrap();
                operation.emitted = true;
                StepOutcome::Complete
            }
            Self::Fusion(operation) => {
                if operation.emitted {
                    return StepOutcome::Complete;
                }
                if let Some(index) = (0..operation.stages.len())
                    .find(|&index| io.input(PortId(index as u16)).is_some())
                {
                    if operation.stages[index].is_some() {
                        return invalid(4);
                    }
                    let Some(input) = bytes.input(PortId(index as u16)) else {
                        return invalid(5);
                    };
                    let Ok(stage) = RetrievalStage::decode(input) else {
                        return invalid(5);
                    };
                    let expected_mechanism = [
                        RetrievalMechanism::VectorSimilarity,
                        RetrievalMechanism::Lexical,
                        RetrievalMechanism::Metadata,
                        RetrievalMechanism::Temporal,
                    ][index];
                    if stage.retriever.mechanism != expected_mechanism {
                        return invalid(6);
                    }
                    io.consume(PortId(index as u16)).unwrap();
                    operation.stages[index] = Some(stage);
                    if operation.stages.iter().any(Option::is_none) {
                        return StepOutcome::Progress;
                    }
                }
                if operation.stages.iter().any(Option::is_none) {
                    return StepOutcome::Await;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                let stages: Vec<_> = operation
                    .stages
                    .iter()
                    .map(|stage| stage.clone().expect("all four stages are present"))
                    .collect();
                let Ok(outcome) = operation.policy.fuse(&stages, None) else {
                    return invalid(7);
                };
                let receipt = HybridRetrievalReceipt {
                    policy_identity: operation.policy.identity.clone(),
                    outcome,
                };
                let Ok(encoded) = receipt.encode() else {
                    return invalid(8);
                };
                if encoded != operation.expected {
                    return invalid(9);
                }
                io.send(PortId(0), operation.output).unwrap();
                operation.emitted = true;
                StepOutcome::Complete
            }
            Self::Sink(_) => {
                let Some(input) = bytes.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                match HybridRetrievalReceipt::decode(input) {
                    Ok(receipt) if receipt.policy_identity == POLICY_IDENTITY => {
                        io.consume(PortId(0)).unwrap();
                        StepOutcome::Complete
                    }
                    _ => invalid(11),
                }
            }
        }
    }
}

type Scheduler = FixedScheduler<TestOperation, HostedValueStore, HostedSignLog, 6, 5, 4, 5, 5, 5>;

fn source_driver(value: ValueRef) -> TestOperation {
    TestOperation::Source(SourceOperation {
        value,
        emitted: false,
    })
}

fn chunk() -> Chunk<ExtractedSourceValue> {
    Chunk::new(
        ExtractionLineage {
            source: SourceRef {
                resource: BoundedResourceRef {
                    identity: ResourceSemanticIdentity::from_digest([7; 32]),
                    content_profile: KindId::from("document/text-utf8@1"),
                    access_class: ResourceClassId::from("resource/read-authorized@1"),
                    extent: ResourceExtent {
                        bytes: 1_024,
                        items: None,
                    },
                    lifetime: ResourceLifetime {
                        version: ResourceVersionIdentity::from_digest([1; 32]),
                        expires_at: None,
                    },
                },
            },
            span: SourceSpan {
                unit: SourceSpanUnit::Bytes,
                start: 0,
                end: 14,
            },
            extraction_profile: "extract/text-utf8@1".into(),
            transform_profiles: vec![],
            parent_chunk: None,
        },
        ExtractedSourceValue::Text(b"project origin".to_vec()),
    )
    .unwrap()
}

fn policy() -> HybridFusionPolicy {
    HybridFusionPolicy {
        identity: POLICY_IDENTITY.into(),
        strategy: FusionStrategy::ReciprocalRank { rank_constant: 60 },
        required_mechanisms: vec![
            RetrievalMechanism::VectorSimilarity,
            RetrievalMechanism::Lexical,
            RetrievalMechanism::Metadata,
            RetrievalMechanism::Temporal,
        ],
        temporal_hard_filter: None,
        maximum_candidates_per_stage: 8,
        maximum_output_candidates: 8,
        maximum_total_work_units: 32,
    }
}

fn stages() -> Vec<RetrievalStage<ExtractedSourceValue>> {
    let mechanisms = [
        RetrievalMechanism::VectorSimilarity,
        RetrievalMechanism::Lexical,
        RetrievalMechanism::Metadata,
        RetrievalMechanism::Temporal,
    ];
    mechanisms
        .into_iter()
        .enumerate()
        .map(|(index, mechanism)| RetrievalStage {
            retriever: RetrieverIdentity {
                identity: format!("retriever/{mechanism:?}@1"),
                mechanism,
            },
            candidates: vec![StageCandidate {
                chunk: chunk(),
                rank: 1,
                score: Some(match mechanism {
                    RetrievalMechanism::VectorSimilarity => {
                        MechanismScore::SimilarityMicros(900_000)
                    }
                    RetrievalMechanism::Lexical => MechanismScore::LexicalScore(42),
                    RetrievalMechanism::Metadata => MechanismScore::MetadataMatch,
                    RetrievalMechanism::Temporal => MechanismScore::TemporalBoundary,
                    RetrievalMechanism::DomainExact => unreachable!(),
                }),
                temporal_evidence_identity: (index == 3).then(|| "project/created".into()),
            }],
            work_units: 1,
        })
        .collect()
}

fn scheduler(stages: &[RetrievalStage<ExtractedSourceValue>], expected: &[u8]) -> Scheduler {
    let mut values = HostedValueStore::new(6, MAX_VALUE_BYTES, 6 * MAX_VALUE_BYTES).unwrap();
    let inputs: Vec<_> = stages
        .iter()
        .map(|stage| values.store(&stage.encode().unwrap()).unwrap())
        .collect();
    let output = values.store(expected).unwrap();
    let mut routes = FixedRoutes::<5, 5>::new(1);
    for index in 0..4 {
        routes
            .install(
                NodeId(index as u16),
                PortId(0),
                RouteRange {
                    start: index as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index as u16),
                    sink: CordEndpoint::local(FUSION_NODE, PortId(index as u16)),
                }],
            )
            .unwrap();
    }
    routes
        .install(
            FUSION_NODE,
            PortId(0),
            RouteRange { start: 4, len: 1 },
            &[RouteTarget {
                cord: CordId(4),
                sink: CordEndpoint::local(SINK_NODE, PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let cord_specs = core::array::from_fn(|index| {
        let (source, sink, sink_port) = if index < 4 {
            (NodeId(index as u16), FUSION_NODE, PortId(index as u16))
        } else {
            (FUSION_NODE, SINK_NODE, PortId(0))
        };
        CordSpec::local(
            CordId(index as u16),
            (source, PortId(0)),
            (sink, sink_port),
            CordCapacity {
                slot_start: index as u16,
                item_capacity: 1,
                byte_capacity: MAX_VALUE_BYTES,
                pressure_policy: Default::default(),
            },
        )
    });
    let source_node = NodeSpec {
        input_cords: [None; 4],
        maximum_step_work: 2,
    };
    let drivers = [
        source_driver(inputs[0]),
        source_driver(inputs[1]),
        source_driver(inputs[2]),
        source_driver(inputs[3]),
        TestOperation::Fusion(Box::new(FusionOperation {
            policy: policy(),
            stages: core::array::from_fn(|_| None),
            expected: expected.to_vec(),
            output,
            emitted: false,
        })),
        TestOperation::Sink(SinkOperation),
    ];
    let signs =
        HostedSignLog::new(128, (128 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    FixedScheduler::new(
        [
            source_node,
            source_node,
            source_node,
            source_node,
            NodeSpec {
                input_cords: [
                    Some(CordId(0)),
                    Some(CordId(1)),
                    Some(CordId(2)),
                    Some(CordId(3)),
                ],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(4)), None, None, None],
                maximum_step_work: 2,
            },
        ],
        cord_specs,
        routes,
        drivers,
        values,
        signs,
    )
    .unwrap()
}

#[test]
fn exact_policy_fuses_four_canonical_inputs_through_the_production_kernel_and_sign() {
    let plan = hybrid_plan::exact_hybrid_plan(POLICY_IDENTITY, MAX_VALUE_BYTES);
    assert!(verify_plan(&plan));
    let planned_policy = plan.fragments[0].placements[0]
        .configuration
        .iter()
        .find(|entry| entry.key == "policy")
        .map(|entry| &entry.value);
    assert_eq!(
        planned_policy,
        Some(&ConfigurationValue::Text(POLICY_IDENTITY.into()))
    );

    let stages = stages();
    let receipt = HybridRetrievalReceipt {
        policy_identity: POLICY_IDENTITY.into(),
        outcome: policy().fuse(&stages, None).unwrap(),
    };
    let encoded = receipt.encode().unwrap();
    let mut scheduler = scheduler(&stages, &encoded);
    scheduler.run(128).unwrap();
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::BackCompleted));
    assert!(scheduler.signs().events().any(|event| {
        event.node == FUSION_NODE && event.kind == KernelEventKind::ValueConsumed
    }));

    let play = bind_active_play(
        &plan.plan_id,
        &plan.fragments[0].host_id,
        &plan.fragments[0].boot_id,
        1,
    );
    let sign = bind_sign(&play.host_id, &play.boot_id, Some(&play.active_play_id), 1);
    assert_eq!(play.plan_id, plan.plan_id);
    assert_eq!(sign.active_play_id, Some(play.active_play_id));
    assert_eq!(receipt.policy_identity, POLICY_IDENTITY);
}

#[test]
fn cancellation_and_malformed_stage_are_distinct_kernel_terminals() {
    let stages = stages();
    let expected = HybridRetrievalReceipt {
        policy_identity: POLICY_IDENTITY.into(),
        outcome: policy().fuse(&stages, None).unwrap(),
    }
    .encode()
    .unwrap();
    let mut cancelled = scheduler(&stages, &expected);
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.run(128),
        Err(conduit_kernel::scheduler::SchedulerError::Cancelled)
    );

    let mut wrong = stages.clone();
    wrong.swap(0, 1);
    let mut malformed = scheduler(&wrong, &expected);
    assert_eq!(
        malformed.run(128),
        Err(conduit_kernel::scheduler::SchedulerError::BackFailed(
            conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::InvalidLifecycle,
                detail: 6
            }
        ))
    );
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
