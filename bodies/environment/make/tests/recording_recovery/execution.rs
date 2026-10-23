//! Exact ordinary lowered graph; only the external store completion is a CPU
//! fixture. This is neither browser persistence nor a Body start receipt.
use super::*;
use conduit_kernel::{self as k, scheduler::*, ValueStorage};

enum Driver {
    Source(RecordingReferenceSource),
    Read(HostCallBack),
    Singleton(RecordingSingletonBack),
    Recovery(Box<RecordingRecoveryBack>),
}
impl StepBack<1> for Driver {
    fn step(&mut self, io: &mut StepIo<1>, bytes: &StepInputBytes<'_, 1>) -> StepOutcome {
        match self {
            Self::Source(b) => b.step(io, bytes),
            Self::Read(b) => b.step(io, bytes),
            Self::Singleton(b) => b.step(io, bytes),
            Self::Recovery(b) => b.step(io, bytes),
        }
    }
    fn prepared_output(&self, port: k::PortId) -> Option<&[u8]> {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<1>>::prepared_output(b, port),
            _ => None,
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<1>>::step_committed(b),
            Self::Read(b) => <HostCallBack as StepBack<1>>::step_committed(b),
            Self::Singleton(b) => <RecordingSingletonBack as StepBack<1>>::step_committed(b),
            Self::Recovery(b) => <RecordingRecoveryBack as StepBack<1>>::step_committed(b),
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(b) => <RecordingReferenceSource as StepBack<1>>::cancel(b),
            Self::Read(b) => <HostCallBack as StepBack<1>>::cancel(b),
            Self::Singleton(b) => <RecordingSingletonBack as StepBack<1>>::cancel(b),
            Self::Recovery(b) => <RecordingRecoveryBack as StepBack<1>>::cancel(b),
        }
    }
}

#[test]
fn lowered_recovery_requires_the_selected_read_before_committing_history() {
    execute(k::HostCallDisposition::Completed, false);
}
#[test]
fn lowered_recovery_refuses_failed_cancelled_and_invalid_reads() {
    execute(k::HostCallDisposition::Failed, false);
    execute(k::HostCallDisposition::Cancelled, false);
    execute(k::HostCallDisposition::Completed, true);
}
fn execute(disposition: k::HostCallDisposition, invalid: bool) {
    let contract = ObservationContract::prepare().unwrap();
    let mut chunk = PreparedRecordingChunk::prepare(context(), &contract).unwrap();
    chunk
        .append(1, 10, RecordingDisposition::CaptureTimeout, None)
        .unwrap();
    chunk
        .append(3, 20, RecordingDisposition::Checksum, None)
        .unwrap();
    let content = if invalid { "{}" } else { chunk.seal().unwrap() }.to_owned();
    let mut reference = reference();
    reference.extent.bytes = content.len() as u64;
    let encoded = reference.encode().unwrap();
    let source = RecordingRecoverySource::compile(&reference).unwrap();
    let (mut host, grant) = fixture_store(&reference);
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    let prepared = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    let fragment = &prepared.plan().fragments[0];
    let lowered = prepared.lowered();
    let node_for = |kind: &str| {
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == kind)
            .unwrap();
        lowered
            .identity
            .node_for_placement(&placement.placement_id)
            .unwrap()
    };
    let read_node = node_for(conduit_semantic_catalog::SNAPSHOT_READ_KIND);
    let recovery_node = node_for("environment/recover-recording");
    let drivers = core::array::from_fn(|i| {
        let placement = &lowered.nodes[i].placement_id;
        let gear = fragment
            .placements
            .iter()
            .find(|p| &p.placement_id == placement)
            .unwrap();
        match gear.kind_id.as_str() {
            "environment/recording-generation" => {
                Driver::Source(generations.take_selected(fragment, placement).unwrap())
            }
            "flow/singleton" => {
                Driver::Singleton(singleton.take_selected(fragment, placement).unwrap())
            }
            "environment/recover-recording" => Driver::Recovery(Box::new(
                recovery.take_selected(fragment, placement).unwrap(),
            )),
            conduit_semantic_catalog::SNAPSHOT_READ_KIND => Driver::Read(HostCallBack::new(512)),
            other => panic!("unexpected selected kind: {other}"),
        }
    });
    let nodes = core::array::from_fn(|i| {
        let spec = lowered.node_specs[i];
        assert!(spec.input_cords[1..].iter().all(Option::is_none));
        NodeSpec {
            input_cords: [spec.input_cords[0]],
            maximum_step_fuel: spec.maximum_step_fuel,
        }
    });
    let cords = core::array::from_fn(|i| lowered.cords[i].spec);
    let mut routes = k::FixedRoutes::<4, 3>::new(1);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut calls = k::FixedHostCallBindings::<4>::new(1);
    for call in &lowered.host_calls {
        calls.install(call.node, call.binding).unwrap();
    }
    calls.seal().unwrap();
    let mut kernel = FixedScheduler::<_, _, _, 4, 3, 1, 3, 4, 3, 4, 4>::new_with_host_calls(
        nodes,
        cords,
        routes,
        calls,
        drivers,
        k::FixedValueStore::<4, 4096>::new(4 * 4096).unwrap(),
        k::FixedSignLog::<128>::new((128 * core::mem::size_of::<k::KernelEvent>()) as u32).unwrap(),
    )
    .unwrap();
    let recovered = |kernel: &FixedScheduler<_, _, _, 4, 3, 1, 3, 4, 3, 4, 4>| {
        let Driver::Recovery(back) = &kernel.drivers()[usize::from(recovery_node.0)] else {
            unreachable!()
        };
        back.recovered()
    };
    let mut requests = 0;
    let mut terminal = false;
    let successful = disposition == k::HostCallDisposition::Completed && !invalid;
    let ((), memory) = allocation_probe::observe(|| {
        for _ in 0..80 {
            if let Some(request) = kernel.next_host_request() {
                requests += 1;
                assert_eq!(request.node, read_node);
                assert_eq!(kernel.values().get(request.input.value).unwrap(), encoded);
                assert_eq!(recovered(&kernel), RecoveredHistory::default());
                // A pending read cannot advance recovery, even when other nodes run.
                for _ in 0..3 {
                    kernel.step().unwrap();
                }
                assert_eq!(recovered(&kernel), RecoveredHistory::default());
                let output = (disposition == k::HostCallDisposition::Completed).then(|| {
                    let value = kernel.store_host_value(content.as_bytes()).unwrap();
                    k::BoundedValueRef::new(value, 4096).unwrap()
                });
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition,
                            output,
                            failure: (disposition == k::HostCallDisposition::Failed).then_some(
                                k::Failure {
                                    code: k::FailureCode::HostCallFailed,
                                    detail: 1,
                                },
                            ),
                        },
                    )
                    .unwrap();
            }
            match kernel.step() {
                Ok(SchedulerStatus::Drained) => {
                    assert!(successful);
                    terminal = true;
                    break;
                }
                Err(SchedulerError::BackFailed(_)) => {
                    assert!(!successful);
                    terminal = true;
                    break;
                }
                Err(error) => panic!("unexpected kernel refusal: {error:?}"),
                _ => {}
            }
        }
    });
    assert!(terminal);
    assert_eq!(requests, 1);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(
        recovered(&kernel),
        if successful {
            RecoveredHistory {
                chunks: 1,
                samples: 2,
                first_sequence: Some(1),
                last_sequence: Some(3),
            }
        } else {
            RecoveredHistory::default()
        }
    );
}
