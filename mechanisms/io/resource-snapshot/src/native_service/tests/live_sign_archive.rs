//! Actual record-service Back, bound native operations and NOR, beside a
//! synthetic sampling Host Call. This is not the complete authored Source,
//! Root admission, a DHT measurement, transport, durable remote custody or HIL.
use super::{fixture, *};
use crate::native_append::service_back::{
    RecordServiceBack, RecordServiceOperation, RecordServiceShape,
};
use alloc::{boxed::Box, vec::Vec};
use conduit_kernel::{
    invocation_archive::ArchivedSign,
    live_sign_archive::{ArchiveLimits, BoxedLiveSignLog, TransferRefusal},
    scheduler::{
        AssignedTerminalTransduction, CordCapacity, CordSpec, FixedScheduler, HostCallRequest,
        NodeSpec, SchedulerStatus, StepBack, StepInputBytes, StepIo, StepOutcome,
    },
    BoundedValueRef, CordEndpoint, CordId, FixedHostCallBindings, FixedRoutes, HostCallBinding,
    HostCallDisposition, HostCallOutcome, HostedSignLog, HostedValueStore, KernelEvent, PortId,
    RemoteEndpointId, RouteRange, RouteTarget, SignSink, ValueStorage,
};

const HISTORY: u32 = 1264;
const APPEND_OPERATIONS: u32 = 970;
const READ_OPERATIONS: u32 = 8192;
const READ_BYTES: usize = crate::read_result::READ_RESULT_BYTES;
const APPEND_BYTES: u32 = crate::append_result::APPEND_RESULT_BYTES;
type Evidence = BoxedLiveSignLog<128, 16, 2>;
type Kernel = FixedScheduler<Back, HostedValueStore, Evidence, 3, 4, 2, 4, 6, 3, 6, 2>;

#[path = "live_sign_archive/backs.rs"]
mod backs;
use backs::{Back, Sampling, Sink};
#[path = "live_sign_archive/workload.rs"]
mod workload;
use workload::Workload;
#[path = "live_sign_archive/dictionary.rs"]
mod dictionary;

fn payload(index: u32) -> [u8; 200] {
    let mut record = [0; 200];
    for (at, byte) in record.iter_mut().enumerate() {
        *byte = index.wrapping_add(at as u32) as u8;
    }
    record[..4].copy_from_slice(&index.to_le_bytes());
    record
}
fn evidence() -> Evidence {
    // Exactly the current finite duplex law: 55 original local slots, six
    // additional remote events, at most one request before explicit transfer.
    let active = HostedSignLog::new_with_remote_storage(
        61,
        61 * core::mem::size_of::<KernelEvent>() as u32,
        6,
        conduit_kernel::remote_sign_storage_bytes(6).unwrap(),
    )
    .unwrap();
    assert!(active.reserved_storage_bytes() <= 24576);
    assert!(HostedSignLog::canonical_logical_storage_bytes(61, 6).unwrap() <= 24576);
    println!("ACTUAL_SERVICE_ARCHIVE_STORAGE_ROW {{\"archive_backing_bytes\":{},\"transfer_overlap_bytes\":{},\"scope\":\"CPU64 owned backing; excludes allocator headers, caller, stack and Root\"}}",
        Evidence::required_archive_backing_bytes(), Evidence::required_transfer_overlap_bytes(&active).unwrap());
    Evidence::try_prepare(
        active,
        ArchiveLimits {
            retained_events: 32768,
            retained_bytes: 2 * 1024 * 1024,
            archive_backing_bytes: 8192,
            transfer_overlap_bytes: 12288,
        },
    )
    .unwrap()
}

#[test]
fn actual_service_1264_steady_reads_preserve_exact_records_and_signs() {
    let mut workload = Workload::new();
    for index in 0..HISTORY {
        workload.read(index, false);
        workload.archive().unwrap();
    }
    assert_eq!(
        workload
            .kernel
            .signs()
            .events_with_remote()
            .collect::<Vec<_>>(),
        workload.expected
    );
    assert_eq!(
        workload.service.inspection(1).unwrap().completed_operations,
        HISTORY
    );
    assert_eq!(
        workload.service.inspection(0).unwrap().completed_operations,
        0
    );
    workload.report("steady", HISTORY, None);
}

#[test]
fn periodic_actual_appends_measure_candidate_fit_or_exact_atomic_refusal() {
    let mut workload = Workload::new();
    let mut completed = 0;
    let mut refusal = None;
    for index in 0..HISTORY {
        workload.read(index, index % 64 == 0);
        completed = index + 1;
        if let Err(reason) = workload.archive() {
            refusal = Some(reason);
            break;
        }
    }
    assert_eq!(
        workload
            .kernel
            .signs()
            .events_with_remote()
            .collect::<Vec<_>>(),
        workload.expected
    );
    workload.report("periodic-append-every-64-reads", completed, refusal);
    if let Some(reason) = refusal {
        assert_eq!(reason, TransferRefusal::ArchiveCapacity);
        // No more remote requests after archive refusal. Local sampling/append
        // still makes real progress; active mandatory evidence is retained.
        let mandatory: Vec<_> = workload
            .kernel
            .signs()
            .events_with_remote()
            .filter(|entry| entry.remote.is_some())
            .collect();
        let samples = workload.samples;
        for _ in 0..3 {
            workload.sample();
        }
        assert_eq!(workload.samples, samples + 3);
        println!("ACTUAL_SERVICE_AFTER_REFUSAL_ROW {{\"reads_at_refusal\":{completed},\"samples_at_refusal\":{samples},\"additional_completed_local_appends\":3,\"samples_after_refusal\":{},\"read_operations_unchanged\":{}}}",
            workload.samples, workload.service.inspection(1).unwrap().completed_operations==completed);
        assert_eq!(
            workload
                .kernel
                .signs()
                .events_with_remote()
                .filter(|entry| entry.remote.is_some())
                .collect::<Vec<_>>(),
            mandatory
        );
    } else {
        assert_eq!(completed, HISTORY);
    }
}
