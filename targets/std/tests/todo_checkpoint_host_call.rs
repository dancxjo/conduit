use conduit_core::*;
use conduit_kernel::scheduler::{
    CordCapacity, CordSpec, FixedScheduler, NodeSpec, StepBack, StepInputBytes, StepIo, StepOutcome,
};
use conduit_kernel::{
    CordId, Failure, FailureCode, FixedHostCallBindings, FixedRoutes, HostCallBinding,
    HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, NodeId,
    PortId, RouteRange, RouteTarget, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use conduit_std_host::todo_checkpoint_call::TodoCheckpointBack;
use conduit_std_host::todo_checkpoint_call::TodoCheckpointHost;
use conduit_std_host::todo_durable_resource::{
    CheckpointIdentity, MissingV2Disposition, SelectedTodoResidence, AUTHORITY_CONTRACT,
    CHECKPOINT_MAX_BYTES, PUBLISH_OPERATION, READ_OPERATION,
};
use conduit_todo_plot::{TodoCommand, TodoState, STATE_MAX_BYTES};
use std::sync::{Arc, Mutex};

enum Driver {
    Source { value: ValueRef, sent: bool },
    Checkpoint(TodoCheckpointBack),
    Sink(Arc<Mutex<Vec<u8>>>),
}
impl StepBack<2> for Driver {
    fn step(&mut self, io: &mut StepIo<2>, inputs: &StepInputBytes<'_, 2>) -> StepOutcome {
        match self {
            Self::Source { value, sent } => {
                if *sent {
                    return StepOutcome::Complete;
                }
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                io.send(PortId(0), *value).unwrap();
                *sent = true;
                StepOutcome::Progress
            }
            Self::Checkpoint(back) => back.step(io, inputs),
            Self::Sink(observed) => {
                if io.input(PortId(0)).is_some() {
                    observed
                        .lock()
                        .unwrap()
                        .extend_from_slice(inputs.input(PortId(0)).unwrap());
                    io.consume(PortId(0)).unwrap();
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    io.consume_closed(PortId(0)).unwrap();
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }
}

fn scheduler(
    observed: Arc<Mutex<Vec<u8>>>,
) -> FixedScheduler<Driver, HostedValueStore, HostedSignLog, 3, 2, 2, 2, 6, 2, 3, 1> {
    let state = TodoState::new("Groceries".into())
        .unwrap()
        .apply(&TodoCommand::Add {
            text: "Buy milk".into(),
        })
        .unwrap();
    let bytes = state.encode_info().unwrap();
    let mut values =
        HostedValueStore::new(4, STATE_MAX_BYTES as u32, (4 * STATE_MAX_BYTES) as u32).unwrap();
    let value = values.store(&bytes).unwrap();
    let mut routes = FixedRoutes::<6, 2>::new(2);
    for (node, cord, sink) in [(0, 0, 1), (1, 1, 2)] {
        routes
            .install(
                NodeId(node),
                PortId(0),
                RouteRange {
                    start: cord,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(cord),
                    sink: conduit_kernel::CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut calls = FixedHostCallBindings::<3>::new(1);
    calls
        .install(
            NodeId(1),
            HostCallBinding {
                call: HostCallId(0),
                maximum_input_bytes: STATE_MAX_BYTES as u32,
                maximum_output_bytes: STATE_MAX_BYTES as u32,
            },
        )
        .unwrap();
    calls.seal().unwrap();
    let charge = core::mem::size_of::<conduit_kernel::KernelEvent>() as u32;
    let signs = HostedSignLog::new(64, charge * 64).unwrap();
    FixedScheduler::new_with_host_calls(
        [
            NodeSpec {
                input_cords: [None, None],
                maximum_step_fuel: 3,
            },
            NodeSpec {
                input_cords: [Some(CordId(0)), None],
                maximum_step_fuel: 3,
            },
            NodeSpec {
                input_cords: [Some(CordId(1)), None],
                maximum_step_fuel: 3,
            },
        ],
        [
            CordSpec::local(
                CordId(0),
                (NodeId(0), PortId(0)),
                (NodeId(1), PortId(0)),
                CordCapacity {
                    slot_start: 0,
                    item_capacity: 1,
                    byte_capacity: STATE_MAX_BYTES as u32,
                    pressure_policy: Default::default(),
                },
            ),
            CordSpec::local(
                CordId(1),
                (NodeId(1), PortId(0)),
                (NodeId(2), PortId(0)),
                CordCapacity {
                    slot_start: 1,
                    item_capacity: 1,
                    byte_capacity: STATE_MAX_BYTES as u32,
                    pressure_policy: Default::default(),
                },
            ),
        ],
        routes,
        calls,
        [
            Driver::Source { value, sent: false },
            Driver::Checkpoint(TodoCheckpointBack::new()),
            Driver::Sink(observed),
        ],
        values,
        signs,
    )
    .unwrap()
}

#[test]
fn checkpoint_output_waits_for_host_call_and_failure_never_reaches_sink() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut kernel = scheduler(observed.clone());
    let request = loop {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            break request;
        }
    };
    assert!(observed.lock().unwrap().is_empty());
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(Failure {
                    code: FailureCode::HostCallFailed,
                    detail: 2,
                }),
            },
        )
        .unwrap();
    for _ in 0..8 {
        let _ = kernel.step();
    }
    assert!(observed.lock().unwrap().is_empty());
}

#[test]
fn completed_host_call_releases_exact_state_to_sink() {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut kernel = scheduler(observed.clone());
    let request = loop {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            break request;
        }
    };
    assert!(observed.lock().unwrap().is_empty());
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(request.input),
                failure: None,
            },
        )
        .unwrap();
    for _ in 0..8 {
        let _ = kernel.step();
    }
    assert_eq!(
        TodoState::decode_info(&observed.lock().unwrap())
            .unwrap()
            .items[0]
            .text,
        "Buy milk"
    );
}

fn placement(host: &str, boot: &str, write: bool) -> PlannedGear {
    let host: HostId = host.into();
    let boot: BootId = boot.into();
    let operation = if write {
        PUBLISH_OPERATION
    } else {
        READ_OPERATION
    };
    let kind = if write {
        conduit_todo_plot::todo_checkpoint_kind().kind_id
    } else {
        kind_id("todo/checkpoint-read")
    };
    let capability: CapabilityId = if write {
        "std-todo-checkpoint-v1".into()
    } else {
        operation.into()
    };
    planned_gear_from_parts! {
        semantic_contract: Default::default(),
        placement_id: "checkpoint".into(), gear_id: "checkpoint".into(),
        kind_id: kind.clone(), kind_contract_revision: "todo/checkpoint@1".into(),
        execution_profile_id: if write { conduit_std_offers::TODO_CHECKPOINT_PROFILE.into() } else { "std/todo-checkpoint@1".into() }, configuration: Default::default(),
        host_id: host.clone(), boot_id: boot.clone(), offer_generation: OfferGeneration(1),
        capability_id: capability.clone(), implementation_id: if write { conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION.into() } else { "std/todo-checkpoint@1".into() },
        artifact_id: if write { conduit_std_offers::TODO_CHECKPOINT_ARTIFACT.into() } else { "std/todo-checkpoint@1".into() }, base: None,
        realization_characteristics: Vec::new(),
        limits: CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 4096 },
        inputs: Vec::new(), outputs: Vec::new(), terminal_transductions: Vec::new(),
        host_calls: vec![HostCallRequirement {
            contract_id: operation.into(), target_kind: Some(kind.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: if write { 4096 } else { 0 },
            maximum_output_bytes: if write { 4096 } else { STATE_MAX_BYTES as u32 },
        }],
        resources: vec![ResourceBinding {
            pool_id: "shared-checkpoint".into(), class_id: "resource/todo-checkpoint@1".into(),
            units: 1, compute: None, protected: None,
            content: Some(ResourceContentOffer {
                contract: ResourceContentRequirement {
                    identity: ResourceSemanticIdentity::from_digest([1; 32]),
                    version: ResourceVersionIdentity::from_digest([2; 32]),
                    content_profile: kind_id("conduit.todo/checkpoint-envelope@1"),
                    maximum_bytes: CHECKPOINT_MAX_BYTES as u32, maximum_items: 1,
                    retention: ResourceRetention::ExternalDurable,
                    sharing: ResourceSharing::SingleWriterPublished,
                    access: if write { ResourceAccessMode::WriteCandidatePublish } else { ResourceAccessMode::ReadPublished },
                    generation_slots: 1, reader_leases: 1, publication_slots: u16::from(write),
                    sensitive: false,
                },
                owner_host: host.clone(), owner_boot: boot.clone(),
                base_id: "std/explicit-shared-checkpoint".into(),
                residence_profile: kind_id("std/explicit-shared-checkpoint@1"),
            }),
        }],
        authority: vec![AuthorityBinding {
            grant_id: format!("grant/{}/{}/{operation}", host.as_str(), boot.as_str()).into(),
            contract_id: AUTHORITY_CONTRACT.into(), host_call_contract_id: operation.into(),
            subject_kind: kind, host_id: host, boot_id: boot, capability_id: capability,
        }],
        pool_references: Vec::new(),
    }
}

#[test]
fn selected_host_call_commits_before_completion_and_second_host_recovers() {
    let root = std::env::temp_dir().join(format!(
        "todo-checkpoint-call-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let identity = CheckpointIdentity {
        body: "body-1".into(),
        plot: "checked-plot-1".into(),
        workload: "revision-1".into(),
        missing_v2: MissingV2Disposition::StartNewList,
    };
    let write = placement("host-a", "boot-a", true);
    let read = placement("host-b", "boot-b", false);
    let lowered = KernelIdentityMap {
        plan_id: "plan-1".into(),
        fragment_id: "fragment-1".into(),
        placements: vec![(NodeId(1), write.placement_id.clone())],
        ports: Vec::new(),
        connections: Vec::new(),
        remote_endpoints: Vec::new(),
        fore_endpoints: Vec::new(),
        host_calls: vec![(NodeId(1), HostCallId(0), PUBLISH_OPERATION.into())],
        resources: Vec::new(),
        resource_connections: Vec::new(),
    };
    let host = TodoCheckpointHost::prepare(&root, &write, &lowered, identity.clone()).unwrap();
    let reader = SelectedTodoResidence::prepare(&root, &read, identity).unwrap();
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut kernel = scheduler(observed.clone());
    let request = loop {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            break request;
        }
    };
    assert!(observed.lock().unwrap().is_empty());
    let bytes = kernel.host_value(request.input.value).unwrap();
    let wrong_request = conduit_kernel::scheduler::HostCallRequest {
        request: conduit_kernel::RequestId(1),
        ..request
    };
    assert_eq!(
        host.perform(wrong_request, bytes).disposition,
        HostCallDisposition::Denied
    );
    assert!(reader.recover(&read.authority[0]).is_err());
    let outcome = host.perform(request, bytes);
    assert_eq!(outcome.disposition, HostCallDisposition::Completed);
    let committed = reader.recover(&read.authority[0]).unwrap();
    assert_eq!(committed.items[0].text, "Buy milk");
    assert!(observed.lock().unwrap().is_empty());
    kernel
        .complete_host_call(request.node, request.request, outcome)
        .unwrap();
    for _ in 0..8 {
        let _ = kernel.step();
    }
    assert_eq!(
        TodoState::decode_info(&observed.lock().unwrap()).unwrap(),
        committed
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn checkpoint_host_advertises_exact_selected_version_before_ledger() {
    let root = std::env::temp_dir().join(format!(
        "todo-checkpoint-offer-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let content = placement("host-a", "boot-a", true).resources[0]
        .content
        .as_ref()
        .unwrap()
        .contract
        .clone();
    let host = conduit_std_host::StdHost::new_for_todo_checkpoint_once(
        conduit_std_host::StdHostConfig {
            host_id: "host-a".into(),
            boot_id: "boot-a".into(),
            offer_generation: OfferGeneration(1),
        },
        &root,
        content.clone(),
    )
    .unwrap();
    let resource = host
        .advertisement()
        .resources
        .iter()
        .find(|offer| offer.pool_id.as_str() == "std/todo-checkpoint")
        .unwrap();
    assert_eq!(resource.content.as_ref().unwrap().contract, content);
    assert!(host.advertisement().capabilities.iter().any(|offer| {
        offer.implementation.implementation_id.as_str()
            == conduit_std_offers::TODO_CHECKPOINT_IMPLEMENTATION
    }));
    std::fs::remove_dir_all(root).unwrap();
}
