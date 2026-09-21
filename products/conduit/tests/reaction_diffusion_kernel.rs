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
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo,
        StepOutcome,
    },
    BoundedValueRef, CordEndpoint, CordId, Failure, FailureCode, FixedHostCallBindings,
    FixedRoutes, HostCallBinding, HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog,
    HostedValueStore, KernelEvent, KernelEventKind, NodeId, PortId, RequestId, RouteRange,
    RouteTarget, SignQuery, ValueRef, ValueStorage,
};
use conduit_std_host::{evolve_reaction_diffusion_hosted, reaction_diffusion_std_offer};

const SOURCE_NODE: NodeId = NodeId(0);
const EVOLVE_NODE: NodeId = NodeId(1);
const OPERATION: HostCallId = HostCallId(0);
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

impl StepBack<1> for TestOperation {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source { value, emitted } => {
                if *emitted {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *emitted = true;
                StepOutcome::Complete
            }
            Self::Evolve {
                pending,
                generation,
            } => {
                if let Some((request, outcome)) = io.host_completion() {
                    if request != REQUEST
                        || !*pending
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                    {
                        return invalid(5);
                    }
                    let (Some(_), Some(canonical)) = (outcome.output, bytes.host_output()) else {
                        return invalid(6);
                    };
                    let Ok(state) = ReactionDiffusionFieldState::decode(canonical) else {
                        return invalid(7);
                    };
                    io.consume_host_completion().unwrap();
                    *pending = false;
                    *generation = Some(state.generation);
                    return StepOutcome::Complete;
                }
                if *pending {
                    return StepOutcome::Await;
                }
                let Some(value) = io.input(PortId(0)) else {
                    return StepOutcome::Await;
                };
                let Some(canonical) = bytes.input(PortId(0)) else {
                    return invalid(3);
                };
                if decode_input(canonical).is_err() {
                    return invalid(3);
                }
                io.consume(PortId(0)).unwrap();
                io.request_host_call(
                    REQUEST,
                    OPERATION,
                    BoundedValueRef::new(value, MAX_INPUT_BYTES).unwrap(),
                )
                .unwrap();
                *pending = true;
                StepOutcome::Progress
            }
        }
    }
}

type Scheduler =
    FixedScheduler<TestOperation, HostedValueStore, HostedSignLog, 2, 1, 1, 1, 1, 1, 2, 1>;

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
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(output_ref, REACTION_DIFFUSION_MAXIMUM_STATE_BYTES)
                        .unwrap(),
                ),
                failure: None,
            },
        )
        .unwrap();
    scheduler.run(16).unwrap();

    let TestOperation::Evolve { generation, .. } = &scheduler.drivers()[1] else {
        panic!("evolution operation identity changed");
    };
    assert_eq!(*generation, Some(3));
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::HostCallCompleted));
    assert!(scheduler
        .signs()
        .contains_kind(KernelEventKind::BackCompleted));
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
        bases: vec![],
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
fn cancellation_prevents_the_admitted_host_call_from_becoming_evolution() {
    let mut scheduler = scheduler();
    let request = next_request(&mut scheduler);
    scheduler.cancel().unwrap();
    assert_eq!(scheduler.pending_host_call_count(), 0);
    assert_eq!(
        scheduler.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        ),
        Err(conduit_kernel::scheduler::SchedulerError::HostCallCompletionRejected)
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
    let mut bindings = FixedHostCallBindings::<2>::new(1);
    bindings
        .install(
            EVOLVE_NODE,
            HostCallBinding {
                call: OPERATION,
                maximum_input_bytes: MAX_INPUT_BYTES,
                maximum_output_bytes: REACTION_DIFFUSION_MAXIMUM_STATE_BYTES,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let signs = HostedSignLog::new(32, (32 * core::mem::size_of::<KernelEvent>()) as u32).unwrap();
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None],
                maximum_step_fuel: 2,
            },
            NodeSpec {
                input_cords: [Some(CordId(0))],
                maximum_step_fuel: 2,
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
                pressure_policy: Default::default(),
            },
        )],
        routes,
        bindings,
        [
            TestOperation::Source {
                value: input_ref,
                emitted: false,
            },
            TestOperation::Evolve {
                pending: false,
                generation: None,
            },
        ],
        values,
        signs,
    )
    .unwrap()
}

fn next_request(scheduler: &mut Scheduler) -> conduit_kernel::scheduler::HostCallRequest {
    for _ in 0..8 {
        if let Some(request) = scheduler.next_host_request() {
            return request;
        }
        scheduler.step().unwrap();
    }
    panic!("reaction-diffusion Host Call was not dispatched")
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

fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
