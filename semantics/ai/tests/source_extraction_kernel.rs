use conduit_ai::{
    extract_source, SourceExtractionLimits, SourceExtractionProfile, SourceExtractionReceipt,
    SourcePayload, SourceRef, SOURCE_READ_AUTHORITY,
};
use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BoundedResourceRef, KindId,
    ResourceDereferenceRequirement, ResourceExtent, ResourceHandleId, ResourceLifetime,
    ResourceReferenceAvailability, ResourceReferenceBinding, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
use conduit_kernel::{
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, SchedulerStatus, StepInputBytes, StepIo,
        StepOperation, StepOutcome,
    },
    BoundedValueRef, CordEndpoint, CordId, Failure, FailureCode, FixedHostCallBindings,
    FixedRoutes, HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog,
    HostedValueStore, KernelEvent, NodeId, PortId, RequestId, RouteRange, RouteTarget, SignSink,
    ValueRef, ValueStorage,
};

const SOURCE_NODE: NodeId = NodeId(0);
const EXTRACTION_NODE: NodeId = NodeId(1);
const SINK_NODE: NodeId = NodeId(2);
const OPERATION: HostCallId = HostCallId(0);
const REQUEST: RequestId = RequestId(1);
const MAX_VALUE_BYTES: u32 = 4096;

#[derive(Clone, Copy)]
struct SourceOperation {
    value: ValueRef,
    emitted: bool,
}

#[derive(Clone, Copy)]
struct ExtractionOperation {
    pending: bool,
    emitted: bool,
}

#[derive(Clone, Copy)]
struct SinkOperation;

#[derive(Clone, Copy)]
enum TestOperation {
    Source(SourceOperation),
    Extract(ExtractionOperation),
    Sink(SinkOperation),
}

impl StepOperation<1> for TestOperation {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
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
            Self::Extract(operation) => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != REQUEST || !operation.pending {
                        return invalid(5);
                    }
                    if outcome.disposition == HostCallDisposition::Failed {
                        let Some(failure) = outcome.failure else {
                            return invalid(3);
                        };
                        if outcome.output.is_some() || io.consume_host_completion().is_err() {
                            return invalid(3);
                        }
                        operation.pending = false;
                        return StepOutcome::Fail(failure);
                    }
                    if outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                    {
                        return invalid(3);
                    }
                    let (Some(output), Some(canonical)) = (outcome.output, bytes.host_output())
                    else {
                        return invalid(6);
                    };
                    if SourceExtractionReceipt::decode(canonical).is_err() {
                        return invalid(7);
                    }
                    if !io.output_ready(PortId(0)) {
                        return StepOutcome::Await;
                    }
                    io.consume_host_completion().unwrap();
                    io.send(PortId(0), output.value).unwrap();
                    operation.pending = false;
                    operation.emitted = true;
                    return StepOutcome::Complete;
                }
                if operation.emitted {
                    return StepOutcome::Complete;
                }
                if operation.pending {
                    return StepOutcome::Await;
                }
                let Some(value) = io.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                let Some(canonical) = bytes.input(PortId(0)) else {
                    return invalid(4);
                };
                if BoundedResourceRef::decode(canonical).is_err() {
                    return invalid(4);
                }
                io.consume(PortId(0)).unwrap();
                io.request_host_call(
                    REQUEST,
                    OPERATION,
                    BoundedValueRef::new(
                        value,
                        conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                    )
                    .expect("resource reference fits its portable bound"),
                )
                .unwrap();
                operation.pending = true;
                StepOutcome::Progress
            }
            Self::Sink(_) => {
                let Some(canonical) = bytes.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                if SourceExtractionReceipt::decode(canonical).is_err() {
                    return invalid(9);
                }
                io.consume(PortId(0)).unwrap();
                StepOutcome::Complete
            }
        }
    }
}

type Scheduler =
    FixedScheduler<TestOperation, HostedValueStore, HostedSignLog, 3, 2, 1, 2, 2, 2, 3, 1>;

fn source() -> SourceRef {
    SourceRef {
        resource: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: KindId::from("document/text-utf8@1"),
            access_class: conduit_core::ResourceClassId::from("resource/read-authorized@1"),
            extent: ResourceExtent {
                bytes: 17,
                items: None,
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([3; 32]),
                expires_at: None,
            },
        },
    }
}

fn binding(
    source: &SourceRef,
    availability: ResourceReferenceAvailability,
) -> ResourceReferenceBinding {
    ResourceReferenceBinding {
        identity: source.resource.identity,
        version: source.resource.lifetime.version,
        content_profile: source.resource.content_profile.clone(),
        access_class: source.resource.access_class.clone(),
        handle: ResourceHandleId::from("handle/source-reader/7"),
        authority_contract: AuthorityContractId::from(SOURCE_READ_AUTHORITY),
        authority_grant: AuthorityGrantId::from("grant/source-read/7"),
        maximum_bytes: 4096,
        maximum_items: None,
        availability,
    }
}

fn scheduler(source: &SourceRef) -> Scheduler {
    let mut values = HostedValueStore::new(4, MAX_VALUE_BYTES, 4 * MAX_VALUE_BYTES).unwrap();
    let source_value = values.store(&source.resource.encode().unwrap()).unwrap();
    let mut routes = FixedRoutes::<2, 2>::new(1);
    routes
        .install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(EXTRACTION_NODE, PortId(0)),
            }],
        )
        .unwrap();
    routes
        .install(
            EXTRACTION_NODE,
            PortId(0),
            RouteRange { start: 1, len: 1 },
            &[RouteTarget {
                cord: CordId(1),
                sink: CordEndpoint::local(SINK_NODE, PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut operations = FixedHostCallBindings::<3>::new(1);
    operations
        .install(
            EXTRACTION_NODE,
            HostCallBinding {
                operation: OPERATION,
                maximum_input_bytes: conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                maximum_output_bytes: MAX_VALUE_BYTES,
            },
        )
        .unwrap();
    operations.seal().unwrap();
    let signs = HostedSignLog::new(64, (64 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(1))],
                maximum_step_work: 2,
            },
        ],
        [
            CordSpec::local(
                CordId(0),
                (SOURCE_NODE, PortId(0)),
                (EXTRACTION_NODE, PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: conduit_core::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                    pressure_policy: Default::default(),
                },
            ),
            CordSpec::local(
                CordId(1),
                (EXTRACTION_NODE, PortId(0)),
                (SINK_NODE, PortId(0)),
                CordCapacity {
                    slot_start: 1,
                    item_capacity: 1,
                    byte_capacity: MAX_VALUE_BYTES,
                    pressure_policy: Default::default(),
                },
            ),
        ],
        routes,
        operations,
        [
            TestOperation::Source(SourceOperation {
                value: source_value,
                emitted: false,
            }),
            TestOperation::Extract(ExtractionOperation {
                pending: false,
                emitted: false,
            }),
            TestOperation::Sink(SinkOperation),
        ],
        values,
        signs,
    )
    .unwrap()
}

fn next_request(scheduler: &mut Scheduler) -> conduit_kernel::scheduler::HostCallRequest {
    for _ in 0..32 {
        if let Some(request) = scheduler.next_host_request() {
            return request;
        }
        assert!(matches!(
            scheduler.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
    }
    panic!("extraction request was not dispatched")
}

#[test]
fn admitted_source_executes_through_the_production_kernel() {
    let source = source();
    let mut scheduler = scheduler(&source);
    let request = next_request(&mut scheduler);
    assert_eq!(request.node, EXTRACTION_NODE);
    assert_eq!(request.request, REQUEST);
    let requested =
        BoundedResourceRef::decode(scheduler.values().get(request.input.value).unwrap()).unwrap();
    assert_eq!(requested, source.resource);

    let receipt = extract_source(
        &source,
        &ResourceDereferenceRequirement {
            content_profile: source.resource.content_profile.clone(),
            access_class: source.resource.access_class.clone(),
            authority_contract: AuthorityContractId::from(SOURCE_READ_AUTHORITY),
            maximum_bytes: 4096,
            maximum_items: None,
        },
        &binding(&source, ResourceReferenceAvailability::Available),
        SourceExtractionProfile::TextUtf8 { overlap_bytes: 2 },
        SourceExtractionLimits {
            maximum_source_bytes: 4096,
            maximum_source_items: 32,
            maximum_chunk_bytes: 512,
            maximum_chunks: 16,
            maximum_output_bytes: 8192,
            maximum_work_units: 16384,
        },
        &SourcePayload::Text("alpha βeta gamma".as_bytes().to_vec()),
    )
    .unwrap();
    let encoded = receipt.encode().unwrap();
    let output = scheduler.store_host_value(&encoded).unwrap();
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(output, MAX_VALUE_BYTES).unwrap()),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(32).unwrap();
    assert!(!scheduler.signs().is_empty());
}

#[test]
fn provider_loss_cancellation_and_pressure_remain_distinct_kernel_terminals() {
    let source = source();
    let mut lost = scheduler(&source);
    let request = next_request(&mut lost);
    lost.complete_host_call(
        request.node,
        request.request,
        HostCallOutcome {
            disposition: HostCallDisposition::Failed,
            output: None,
            failure: Some(Failure {
                code: FailureCode::HostCallFailed,
                detail: 1,
            }),
        },
    )
    .unwrap();
    assert_eq!(
        lost.run(32),
        Err(conduit_kernel::scheduler::SchedulerError::BackFailed(
            conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostCallFailed,
                detail: 1
            }
        ))
    );

    let mut cancelled = scheduler(&source);
    let _ = next_request(&mut cancelled);
    cancelled.cancel().unwrap();
    assert_eq!(
        cancelled.run(32),
        Err(conduit_kernel::scheduler::SchedulerError::Cancelled)
    );

    let mut pressured = scheduler(&source);
    let request = next_request(&mut pressured);
    let oversized = vec![0_u8; MAX_VALUE_BYTES as usize + 1];
    assert!(pressured.store_host_value(&oversized).is_err());
    pressured
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::StorageExhausted,
                    detail: 2,
                }),
            },
        )
        .unwrap();
    assert_eq!(
        pressured.run(32),
        Err(conduit_kernel::scheduler::SchedulerError::BackFailed(
            conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::StorageExhausted,
                detail: 2
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
