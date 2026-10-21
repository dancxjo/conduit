#![cfg(target_os = "linux")]
//! Private control fixture: real threads, not installed lane admission evidence.
use conduit_composite::{
    KernelCompositeSignStorage, KernelOperationRegistry, PreparedRegionKernel,
};
use conduit_core::*;
use conduit_kernel::{scheduler::SchedulerStatus, CordId, KernelEventKind};
use conduit_plan_lowering::execution_regions::*;
use std::time::{Duration, Instant};
#[path = "../../../architecture/composite/tests/support/region_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "../src/kernel_preparation/worker.rs"]
pub(crate) mod worker;
mod kernel_preparation {
    pub(crate) use crate::worker;
}
#[allow(dead_code)]
#[path = "../src/kernel_preparation/service/controller.rs"]
mod controller;
use worker::{InitializedRegionWorker, WorkerError, WorkerReply};
const STACK: u32 = 2 * 1024 * 1024;
fn reply(
    worker: &mut InitializedRegionWorker,
    bytes: &mut [u8],
) -> Result<WorkerReply, WorkerError> {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match worker.take_reply(bytes) {
            Ok(Some(reply)) => return reply,
            Ok(None) | Err(WorkerError::Busy) => {}
            Err(error) => return Err(error),
        }
        assert!(Instant::now() < until, "worker reply timed out");
        std::thread::yield_now();
    }
}
fn submit(mut send: impl FnMut() -> Result<(), WorkerError>) {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match send() {
            Ok(()) => return,
            Err(WorkerError::Busy) => {}
            Err(e) => panic!("worker submit: {e:?}"),
        }
        assert!(Instant::now() < until, "worker submission timed out");
        std::thread::yield_now();
    }
}
fn worker_plan() -> Plan {
    let mut fragment = fixture::plan().fragments.remove(0);
    for region in &mut fragment.execution_regions {
        let ExecutionScheduling::PartitionedBoundedStep { storage } = &mut region.scheduling else {
            panic!()
        };
        storage.stack_bytes = STACK;
    }
    seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
}
#[test]
fn private_workers_preserve_mailbox_pressure_delivery_and_retired_evidence() {
    let plan = worker_plan();
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(fixture::Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        })
        .unwrap();
    let mut source = InitializedRegionWorker::spawn(STACK, 2).unwrap();
    let mut sink = InitializedRegionWorker::spawn(STACK, 2).unwrap();
    assert_eq!(source.stack_bytes(), STACK);
    let backing = source.control_backing_bytes().unwrap();
    assert_eq!(source.start(), Err(WorkerError::NotInstalled));
    source
        .install(
            PreparedRegionKernel::prepare(
                &prepared,
                0,
                &registry,
                KernelCompositeSignStorage::default(),
            )
            .unwrap(),
        )
        .unwrap();
    sink.install(
        PreparedRegionKernel::prepare(
            &prepared,
            1,
            &registry,
            KernelCompositeSignStorage::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let mut bytes = [0u8; 2];
    submit(|| source.step());
    assert_eq!(reply(&mut source, &mut bytes), Err(WorkerError::NotStarted));
    source.start().unwrap();
    sink.start().unwrap();
    submit(|| source.step());
    // The first command or its unread reply continues to own the single slot.
    assert_eq!(source.step(), Err(WorkerError::Busy));
    assert!(matches!(
        reply(&mut source, &mut bytes),
        Ok(WorkerReply::Step(SchedulerStatus::Progress { .. }))
    ));
    submit(|| source.offer(CordId(0)));
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        match source.take_reply(&mut []) {
            Err(WorkerError::BufferTooSmall) => break,
            Ok(None) | Err(WorkerError::Busy) => {}
            other => panic!("offer was not retained: {other:?}"),
        }
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
    let WorkerReply::Offer {
        sequence: Some(sequence),
        bytes: len,
    } = reply(&mut source, &mut bytes).unwrap()
    else {
        panic!()
    };
    assert_eq!(&bytes[..len], &[42]);
    submit(|| sink.admit(CordId(0), sequence, &bytes[..len]));
    assert!(matches!(
        reply(&mut sink, &mut []),
        Ok(WorkerReply::Ingress(_))
    ));
    submit(|| source.delivered(CordId(0), sequence));
    assert_eq!(reply(&mut source, &mut []), Ok(WorkerReply::Delivered));
    submit(|| source.offer(CordId(0)));
    assert_eq!(
        reply(&mut source, &mut []),
        Ok(WorkerReply::Offer {
            sequence: None,
            bytes: 0
        })
    );
    submit(|| sink.step());
    assert!(matches!(
        reply(&mut sink, &mut []),
        Ok(WorkerReply::Step(SchedulerStatus::Progress { .. }))
    ));
    assert_eq!(source.control_backing_bytes().unwrap(), backing);
    let retired_source = source.shutdown().unwrap();
    let retired_sink = sink.shutdown().unwrap();
    assert_eq!(retired_source.failure, None);
    assert_eq!(retired_sink.cancellation, Some(Ok(())));
    let retired_source = retired_source.kernel.unwrap();
    let retired_sink = retired_sink.kernel.unwrap();
    assert_eq!(retired_source.plan_id(), &plan.plan_id);
    assert!(retired_sink
        .signs()
        .iter()
        .any(|e| e.kind == KernelEventKind::ValueConsumed));
}

struct PanicBack;
impl conduit_kernel::scheduler::StepBack<16> for PanicBack {
    fn step(
        &mut self,
        _: &mut conduit_kernel::scheduler::StepIo<16>,
        _: &conduit_kernel::scheduler::StepInputBytes<'_, 16>,
    ) -> conduit_kernel::scheduler::StepOutcome {
        panic!("intentional worker failure fixture")
    }
}
struct PanicFactory(fixture::Factory);
impl conduit_composite::KernelOperationFactory for PanicFactory {
    fn implementation_id(&self) -> &ImplementationId {
        self.0.implementation_id()
    }
    fn budget(
        &self,
        placement: &PlannedGear,
    ) -> Result<conduit_composite::KernelOperationBudget, String> {
        self.0.budget(placement)
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut conduit_kernel::HostedValueStore,
    ) -> Result<Box<dyn conduit_kernel::scheduler::StepBack<16> + Send>, String> {
        Ok(Box::new(PanicBack))
    }
    fn prepare_worker(
        &self,
        placement: &PlannedGear,
        values: &mut conduit_kernel::HostedValueStore,
    ) -> Result<conduit_composite::PreparedWorkerBack, String> {
        conduit_composite::PreparedWorkerBack::new(self.prepare(placement, values)?, 0)
    }
}
#[test]
fn panicking_worker_wakes_controller_and_retains_retirement_custody() {
    let plan = worker_plan();
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(PanicFactory(fixture::Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        }))
        .unwrap();
    let kernel = PreparedRegionKernel::prepare(
        &prepared,
        0,
        &registry,
        KernelCompositeSignStorage::default(),
    )
    .unwrap();
    let mut worker = InitializedRegionWorker::spawn(STACK, 2).unwrap();
    worker.install(kernel).unwrap();
    worker.start().unwrap();
    submit(|| worker.step());
    assert!(matches!(
        reply(&mut worker, &mut []),
        Err(WorkerError::Panicked | WorkerError::Poisoned)
    ));
    let retired = worker.shutdown().unwrap();
    assert_eq!(retired.failure, Some(WorkerError::Panicked));
    assert_eq!(retired.cancellation, Some(Ok(())));
    assert_eq!(retired.kernel.unwrap().plan_id(), &plan.plan_id);
}

#[test]
fn finite_controller_delivers_original_cord_and_keeps_backing_fixed() {
    let plan = worker_plan();
    let prepared = PreparedExecutionRegions::prepare(
        &plan,
        &plan.fragments[0].fragment_id,
        ExecutionRegionLimits {
            maximum_regions: 2,
            maximum_lanes: 2,
            maximum_nodes: 2,
            maximum_cords: 1,
        },
    )
    .unwrap();
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(fixture::Factory {
            implementation: plan.fragments[0].placements[0].implementation_id.clone(),
        })
        .unwrap();
    let mut workers: Vec<_> = (0..2)
        .map(|_| InitializedRegionWorker::spawn(STACK, 2).unwrap())
        .collect();
    for (region, worker) in workers.iter_mut().enumerate() {
        worker
            .install(
                PreparedRegionKernel::prepare(
                    &prepared,
                    region,
                    &registry,
                    KernelCompositeSignStorage::default(),
                )
                .unwrap(),
            )
            .unwrap();
        worker.start().unwrap();
    }
    let mut controller =
        controller::RegionController::new(vec![controller::Route::new(CordId(0), 0, 1)], 2);
    let backing = controller.backing_bytes();
    let until = Instant::now() + Duration::from_secs(5);
    let mut rounds = 0;
    while rounds < 4 {
        match controller.advance(&mut workers).unwrap() {
            controller::RegionAdvance::RoundComplete => rounds += 1,
            controller::RegionAdvance::Pending => std::thread::yield_now(),
            controller::RegionAdvance::Progress => {}
        }
        assert!(Instant::now() < until, "finite controller stalled");
    }
    assert_eq!(controller.backing_bytes(), backing);
    let mut retired = workers.into_iter().map(|w| w.shutdown().unwrap());
    let source = retired.next().unwrap().kernel.unwrap();
    let sink = retired.next().unwrap().kernel.unwrap();
    assert_eq!(source.plan_id(), &plan.plan_id);
    assert!(sink
        .signs()
        .iter()
        .any(|e| e.kind == KernelEventKind::ValueConsumed));
    assert_eq!(
        source
            .signs()
            .iter()
            .filter(|e| e.kind == KernelEventKind::RemoteValueDelivered)
            .count(),
        1
    );
    assert!(sink
        .signs()
        .iter()
        .any(|e| e.kind == KernelEventKind::RemoteInputClosed));
    assert!(source
        .remote_signs()
        .iter()
        .all(|(_, identity)| identity.endpoint.0 == 0));
}
