#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_dht11_observation::{ObservationContract, PreparedObservationQuantityForms};
use conduit_environment_runtime::recording::*;
use conduit_kernel::{self as k, scheduler::*};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    LazyLock,
};
static NOW: AtomicU64 = AtomicU64::new(100);
fn now() -> u64 {
    NOW.load(Ordering::Relaxed)
}
fn context() -> RecordingContext<'static> {
    static HOST: LazyLock<c::HostId> = LazyLock::new(|| "host/encoder".into());
    static BOOT: LazyLock<c::BootId> = LazyLock::new(|| "boot/encoder".into());
    static PLAN: LazyLock<c::PlanId> = LazyLock::new(|| "plan/encoder".into());
    static PLAY: LazyLock<c::ActivePlayId> = LazyLock::new(|| "play/encoder".into());
    RecordingContext {
        host: &HOST,
        boot: &BOOT,
        plan: &PLAN,
        play: &PLAY,
    }
}
fn prepared(samples: usize) -> (RecordingEncoderBack, Vec<u8>, ObservationContract) {
    let contract = ObservationContract::prepare().unwrap();
    let reference = c::BoundedResourceRef {
        identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: "text/json-utf8@1".into(),
        access_class: "resource/snapshot@1".into(),
        extent: c::ResourceExtent {
            bytes: PreparedRecordingSnapshot::content_bytes(context(), &contract, samples).unwrap()
                as u64,
            items: Some(1),
        },
        lifetime: c::ResourceLifetime {
            version: c::ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    };
    let back =
        RecordingEncoderBack::prepare(context(), &contract, samples, &reference, 17, now).unwrap();
    let mut codec = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let canonical = codec.unpack(&[0; 4]).unwrap().to_vec();
    (back, canonical, contract)
}
fn stage(back: &mut RecordingEncoderBack, canonical: &[u8], fuel: u16) -> StepOutcome {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: canonical.len() as u32,
    };
    let mut io = StepIo::test_frame([Some(value)], [false], [Some(4096)], None, fuel);
    back.step(
        &mut io,
        &StepInputBytes::test_frame([Some(canonical)], None),
    )
}
#[test]
fn only_committed_appends_and_emission_advance_under_retry_pressure_and_cancel() {
    NOW.store(100, Ordering::Relaxed);
    let (mut back, canonical, contract) = prepared(2);
    assert_eq!(stage(&mut back, &canonical, 16), StepOutcome::Progress);
    assert_eq!(back.committed_samples(), 0);
    // A refused retry discards staged work; even a stray commit callback cannot
    // turn its old prepared append into an accepted sample.
    assert!(matches!(
        stage(&mut back, &canonical, 0),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::WorkBudgetExhausted,
            ..
        })
    ));
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    assert_eq!(back.committed_samples(), 0);
    assert_eq!(stage(&mut back, &canonical, 16), StepOutcome::Progress);
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    assert_eq!(back.committed_samples(), 1);
    NOW.store(99, Ordering::Relaxed);
    assert!(matches!(
        stage(&mut back, &canonical, 16),
        StepOutcome::Fail(_)
    ));
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    assert_eq!(back.committed_samples(), 1);
    NOW.store(101, Ordering::Relaxed);
    assert!(matches!(
        stage(&mut back, b"invalid", 16),
        StepOutcome::Fail(_)
    ));
    assert_eq!(stage(&mut back, &canonical, 16), StepOutcome::Progress);
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    let mut pressure = StepIo::test_frame([None], [false], [None], None, 16);
    assert_eq!(
        back.step(&mut pressure, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Await
    );
    assert!(<RecordingEncoderBack as StepBack<1>>::prepared_output(&back, k::PortId(0)).is_none());
    let mut too_small = StepIo::test_frame([None], [false], [Some(1)], None, 16);
    assert!(matches!(
        back.step(&mut too_small, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::StorageExhausted,
            ..
        })
    ));
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    assert_eq!(back.committed_samples(), 2);
    let mut emit = StepIo::test_frame([None], [false], [Some(4096)], None, 16);
    assert_eq!(
        back.step(&mut emit, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    let first = <RecordingEncoderBack as StepBack<1>>::prepared_output(&back, k::PortId(0))
        .unwrap()
        .to_vec();
    let mut reader = PreparedRecordingReader::prepare(context(), &contract).unwrap();
    reader.read(std::str::from_utf8(&first).unwrap()).unwrap();
    assert_eq!(reader.len(), 2);
    assert_eq!(reader.sample(0).unwrap().sequence, 17);
    assert_eq!(reader.sample(1).unwrap().sequence, 18);
    assert_eq!(reader.sample(1).unwrap().sampled_at_boot_ms, 101);
    // No committed output: the same content can be proposed again unchanged.
    let mut retry = StepIo::test_frame([None], [false], [Some(4096)], None, 16);
    assert_eq!(
        back.step(&mut retry, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Progress
    );
    assert_eq!(
        <RecordingEncoderBack as StepBack<1>>::prepared_output(&back, k::PortId(0)).unwrap(),
        first
    );
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    let mut done = StepIo::test_frame([None], [false], [Some(4096)], None, 16);
    assert_eq!(
        back.step(&mut done, &StepInputBytes::test_frame([None], None)),
        StepOutcome::Complete
    );
    let (mut cancelled, canonical, _) = prepared(2);
    assert_eq!(stage(&mut cancelled, &canonical, 16), StepOutcome::Progress);
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut cancelled);
    assert_eq!(stage(&mut cancelled, &canonical, 16), StepOutcome::Progress);
    <RecordingEncoderBack as StepBack<1>>::cancel(&mut cancelled);
    <RecordingEncoderBack as StepBack<1>>::step_committed(&mut cancelled);
    assert_eq!(cancelled.committed_samples(), 1);
    assert!(matches!(
        stage(&mut cancelled, &canonical, 16),
        StepOutcome::Fail(k::Failure {
            code: k::FailureCode::Cancelled,
            ..
        })
    ));
}

#[test]
fn committed_append_seal_and_pressure_retry_allocate_nothing() {
    let (mut back, canonical, _) = prepared(1);
    let ((), allocations) = allocation_probe::observe(|| {
        assert_eq!(stage(&mut back, &canonical, 16), StepOutcome::Progress);
        <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
        for _ in 0..2 {
            let mut io = StepIo::test_frame([None], [false], [Some(4096)], None, 16);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Progress
            );
            assert!(
                <RecordingEncoderBack as StepBack<1>>::prepared_output(&back, k::PortId(0))
                    .is_some()
            );
        }
        <RecordingEncoderBack as StepBack<1>>::step_committed(&mut back);
    });
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
}

#[test]
fn write_backing_accounts_all_capacities_without_unpack_workspaces() {
    let contract = ObservationContract::prepare().unwrap();
    let host: c::HostId = "rp2040/flash/e6639c758f335f34".into();
    let boot: c::BootId = "b".repeat(64).into();
    let plan: c::PlanId = "p".repeat(64).into();
    let play: c::ActivePlayId = "x".repeat(64).into();
    let selected_context = || RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let full_codec = PreparedObservationQuantityForms::prepare(&contract).unwrap();
    let unpack_backing = full_codec.retained_backing_bytes().unwrap();
    assert!(unpack_backing >= 4096);
    for samples in [8, 64] {
        let content_bytes =
            PreparedRecordingSnapshot::content_bytes(selected_context(), &contract, samples)
                .unwrap();
        let reference = c::BoundedResourceRef {
            identity: c::ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: "text/json-utf8@1".into(),
            access_class: "resource/snapshot@1".into(),
            extent: c::ResourceExtent {
                bytes: content_bytes as u64,
                items: Some(1),
            },
            lifetime: c::ResourceLifetime {
                version: c::ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        };
        let chunk = PreparedRecordingChunk::prepare(selected_context(), &contract).unwrap();
        let snapshot =
            PreparedRecordingSnapshot::prepare(selected_context(), &contract, samples, &reference)
                .unwrap();
        // Both fixed String capacities are charged even for an eight-record
        // generation; the fixed 64-record array remains part of inline Self.
        assert_eq!(
            chunk.retained_backing_bytes(),
            Some(2 * MAXIMUM_CHUNK_BYTES)
        );
        let snapshot_bytes = 2 * MAXIMUM_CHUNK_BYTES + reference.encode().unwrap().len();
        assert_eq!(snapshot.retained_backing_bytes(), Some(snapshot_bytes));
        let ((back, inline_bytes), measured) = allocation_probe::observe(|| {
            let back = RecordingEncoderBack::prepare(
                selected_context(),
                &contract,
                samples,
                &reference,
                17,
                now,
            )
            .unwrap();
            (back, core::mem::size_of::<RecordingEncoderBack>())
        });
        let staging_bytes =
            conduit_plot::maximum_prepared_canonical_value_bytes(contract.observation_type())
                .unwrap() as usize;
        let retained = back.retained_backing_bytes().unwrap();
        assert_eq!(retained, snapshot_bytes + staging_bytes + content_bytes);
        assert!(measured.peak_bytes >= retained);
        println!("samples={samples} retained_heap={retained} inline={inline_bytes} constructor_peak={} removed_unpack_heap={}", measured.peak_bytes, 2 * unpack_backing);
    }
}
