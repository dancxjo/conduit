use conduit_alife::{
    install_reaction_diffusion_catalogs, GrayScottParameters, ReactionDiffusionEvolveRequest,
    ReactionDiffusionFieldId, ReactionDiffusionFieldState, REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
    REACTION_DIFFUSION_REQUEST_BYTES,
};
use conduit_core::{
    BaseImplementationId, BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration,
    PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, FixedScheduler, NodeSpec, OperationDriver},
    BoundedValueRef, CordEndpoint, CordId, Failure, FailureCode, FixedHostOperationBindings,
    FixedRoutes, HostOperationBinding, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, HostedSignLog, HostedValueStore, KernelEvent, KernelEventKind, NodeId,
    Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    SignQuery, ValueRef, ValueStorage,
};
use conduit_std_host::{evolve_reaction_diffusion_hosted, reaction_diffusion_std_offer};

const SOURCE_NODE: NodeId = NodeId(0);
const EVOLVE_NODE: NodeId = NodeId(1);
const OPERATION: HostOperationId = HostOperationId(0);
const REQUEST: RequestId = RequestId(1);
const MAX_INPUT_BYTES: u32 =
    4 + REACTION_DIFFUSION_MAXIMUM_STATE_BYTES + REACTION_DIFFUSION_REQUEST_BYTES;
const MAX_VALUE_BYTES: u32 = MAX_INPUT_BYTES;
const FIELD_ID: ReactionDiffusionFieldId = ReactionDiffusionFieldId(*b"field-kernel-001");

#[derive(Clone, Copy)]
enum TestOperation {
    Source {
        value: ValueRef,
        emitted: bool,
    },
    Evolve {
        pending: bool,
        generation: Option<u64>,
    },
}

impl Operation for TestOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source { value, emitted } => {
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: *value,
                }
            }
            Self::Evolve { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, _input: OperationInput) -> OperationAction {
        invalid(1)
    }

    fn resume_value(&mut self, port: PortId, value: ValueRef, bytes: &[u8]) -> OperationAction {
        let Self::Evolve { pending, .. } = self else {
            return invalid(2);
        };
        if port != PortId(0) || *pending || decode_input(bytes).is_err() {
            return invalid(3);
        }
        *pending = true;
        OperationAction::RequestHostOperation {
            request: REQUEST,
            operation: OPERATION,
            input: BoundedValueRef::new(value, MAX_INPUT_BYTES).unwrap(),
        }
    }

    fn resume_host_operation(
        &mut self,
        request: RequestId,
        outcome: HostOperationOutcome,
        canonical: Option<&[u8]>,
    ) -> OperationAction {
        let Self::Evolve {
            pending,
            generation,
        } = self
        else {
            return invalid(4);
        };
        if request != REQUEST
            || !*pending
            || outcome.disposition != HostOperationDisposition::Completed
        {
            return invalid(5);
        }
        let (Some(_), Some(bytes)) = (outcome.output, canonical) else {
            return invalid(6);
        };
        let Ok(state) = ReactionDiffusionFieldState::decode(bytes) else {
            return invalid(7);
        };
        *pending = false;
        *generation = Some(state.generation);
        OperationAction::Complete
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted, .. } if *emitted => {
                *emitted = false;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }
}

type Scheduler = FixedScheduler<
    OperationDriver<TestOperation, 1>,
    HostedValueStore,
    HostedSignLog,
    2,
    1,
    1,
    1,
    1,
    1,
    2,
    1,
>;

#[test]
fn canonical_example_executes_the_hosted_reference_through_the_production_kernel() {
    assert_canonical_example_checks_and_plans();
    let mut scheduler = scheduler();
    let request = next_request(&mut scheduler);
    let input = scheduler.host_value(request.input.value).unwrap();
    let (state, evolution) = decode_input(input).unwrap();
    let output = evolve_reaction_diffusion_hosted(&state, evolution).unwrap();
    let encoded = output.encode().unwrap();
    let output_ref = scheduler.store_host_value(&encoded).unwrap();
    scheduler
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(output_ref, REACTION_DIFFUSION_MAXIMUM_STATE_BYTES)
                        .unwrap(),
                ),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(16).unwrap();

    let TestOperation::Evolve { generation, .. } = scheduler.drivers()[1].operation() else {
        panic!("evolution operation identity changed");
    };
    assert_eq!(*generation, Some(3));
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::HostOperationCompleted));
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::OperationCompleted));
}

fn assert_canonical_example_checks_and_plans() {
    let source = include_str!("../../../proof/fixtures/forms/reaction-diffusion.conduit");
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_reaction_diffusion_catalogs(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "field-step", &profile).unwrap();
    let host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/reaction-diffusion-kernel"),
        boot_id: BootId::from("boot/reaction-diffusion-kernel"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("std/reaction-diffusion-kernel@1"),
        resources: vec![],
        planner_capabilities: vec![],
        capabilities: vec![reaction_diffusion_std_offer()],
    };
    let placements = conduit_planner::default_expanded_placements(
        &expanded.expanded,
        core::slice::from_ref(&host),
    )
    .unwrap();
    let plan = conduit_planner::plan_expanded_canonical(
        &expanded.expanded,
        &[host],
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
    )
    .unwrap();
    assert_eq!(plan.fragments.len(), 1);
    assert_eq!(plan.fragments[0].placements.len(), 1);
}

#[test]
fn cancellation_prevents_the_admitted_host_operation_from_becoming_evolution() {
    let mut scheduler = scheduler();
    let request = next_request(&mut scheduler);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.pending_host_operation_count(), 0);
    assert_eq!(
        scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(conduit_kernel::scheduler::SchedulerError::HostOperationCompletionRejected)
    );
    assert_eq!(
        scheduler.run(16),
        Err(conduit_kernel::scheduler::SchedulerError::Cancelled)
    );
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::CancellationRequested));
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::RunCancelled));
}

fn scheduler() -> Scheduler {
    let input = encode_input();
    let mut values = HostedValueStore::new(4, MAX_VALUE_BYTES, 4 * MAX_VALUE_BYTES).unwrap();
    let input_ref = values.store(&input).unwrap();
    let mut routes = FixedRoutes::<1, 1>::new(1);
    routes
        .install(
            SOURCE_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: CordEndpoint::local(EVOLVE_NODE, PortId(0)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();
    let mut bindings = FixedHostOperationBindings::<2>::new(1);
    bindings
        .install(
            EVOLVE_NODE,
            HostOperationBinding {
                operation: OPERATION,
                maximum_input_bytes: MAX_INPUT_BYTES,
                maximum_output_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let signs = HostedSignLog::new(32, (32 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    FixedScheduler::new_with_host_operations(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_work: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_work: 2,
            },
        ],
        [CordSpec::local(
            CordId(0),
            (SOURCE_NODE, PortId(0)),
            (EVOLVE_NODE, PortId(0)),
            CordCapacity {
                slot_start: 0,
                item_capacity: 1,
                byte_capacity: MAX_INPUT_BYTES,
            },
        )],
        routes,
        bindings,
        [
            OperationDriver::new(TestOperation::Source {
                value: input_ref,
                emitted: false,
            })
            .unwrap(),
            OperationDriver::new(TestOperation::Evolve {
                pending: false,
                generation: None,
            })
            .unwrap(),
        ],
        values,
        signs,
    )
    .unwrap()
}

fn next_request(scheduler: &mut Scheduler) -> conduit_kernel::scheduler::HostOperationRequest {
    for _ in 0..8 {
        if let Some(request) = scheduler.next_host_request() {
            return request;
        }
        scheduler.step().unwrap();
    }
    panic!("reaction-diffusion host operation was not dispatched")
}

fn encode_input() -> Vec<u8> {
    let state = ReactionDiffusionFieldState::initialized(
        FIELD_ID,
        3,
        3,
        GrayScottParameters::REFERENCE,
        17,
    )
    .unwrap()
    .encode()
    .unwrap();
    let request = ReactionDiffusionEvolveRequest {
        field_id: FIELD_ID,
        expected_generation: 0,
        generations: 3,
        admitted_cell_generations: 27,
    }
    .encode();
    let mut encoded = Vec::with_capacity(4 + state.len() + request.len());
    encoded.extend_from_slice(&(state.len() as u32).to_le_bytes());
    encoded.extend_from_slice(&state);
    encoded.extend_from_slice(&request);
    encoded
}

fn decode_input(
    encoded: &[u8],
) -> Result<(ReactionDiffusionFieldState, ReactionDiffusionEvolveRequest), ()> {
    let length = encoded.get(..4).ok_or(())?;
    let state_length = u32::from_le_bytes(length.try_into().map_err(|_| ())?) as usize;
    let state_end = 4_usize.checked_add(state_length).ok_or(())?;
    let state = ReactionDiffusionFieldState::decode(encoded.get(4..state_end).ok_or(())?)
        .map_err(|_| ())?;
    let request = ReactionDiffusionEvolveRequest::decode(encoded.get(state_end..).ok_or(())?)
        .map_err(|_| ())?;
    Ok((state, request))
}

fn invalid(detail: u16) -> OperationAction {
    OperationAction::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
