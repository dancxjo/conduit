//! Actual native append binding, media effect and kernel-custody boundary tests.
extern crate alloc;
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_resource_snapshot::{append::*, native_append::*};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "support/native_append.rs"]
mod fixture;
use fixture::*;
fn decode(bytes: &[u8]) -> AppendOutcome {
    let value = validate_canonical_structured_value(bytes).unwrap();
    assert_eq!(
        value.type_bytes(),
        conduit_resource_snapshot::append_result::result_type()
            .unwrap()
            .canonical_bytes()
            .unwrap()
    );
    match value.variant_tag().unwrap() {
        "recorded" => AppendOutcome::Recorded(DurableRecordIndex(
            decode_count(
                value
                    .variant_payload("recorded")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
            )
            .unwrap() as u32,
        )),
        "capacity" => AppendOutcome::Capacity,
        "uncertain" => AppendOutcome::Uncertain,
        "recovery-required" => AppendOutcome::RecoveryRequired,
        _ => panic!("foreign outcome"),
    }
}
fn invoke(owner: &mut RecordAppendHostCall<Storage>, request: u32, input: &[u8]) -> AppendOutcome {
    let mut out = [0; APPEND_RESULT_BYTES as usize];
    let n = owner
        .invoke(
            NodeId(0),
            HostCallId(0),
            RequestId(request),
            input,
            &mut out,
        )
        .unwrap();
    decode(&out[..n])
}
fn acknowledge(owner: &mut RecordAppendHostCall<Storage>, request: u32, outcome: AppendOutcome) {
    owner
        .acknowledge_kernel_completion(NodeId(0), HostCallId(0), RequestId(request), outcome)
        .unwrap();
}
#[test]
fn exact_native_and_fore_binding_preserves_kernel_commit_gate_and_typed_capacity() {
    for fore in [false, true] {
        let f = Fixture::new();
        assert!(f.bind_mode(true, fore).is_err());
        let mut owner = f.bind_mode(false, fore).unwrap();
        let mut out = [0; APPEND_RESULT_BYTES as usize];
        assert_eq!(
            owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"abc", &mut []),
            Err(AppendCallRefusal::OutputPressure)
        );
        assert_eq!(f.storage.programs.get(), 0);
        let (result, cost) = allocation_probe::observe(|| {
            owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"abc", &mut out)
        });
        let outcome = decode(&out[..result.unwrap()]);
        assert_eq!(outcome, AppendOutcome::Recorded(DurableRecordIndex(0)));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert_eq!(invoke(&mut owner, 0, b"abc"), outcome);
        assert_eq!(
            owner.invoke(NodeId(0), HostCallId(0), RequestId(0), b"abd", &mut out),
            Err(AppendCallRefusal::StaleRequest)
        );
        assert_eq!(
            owner.invoke(NodeId(0), HostCallId(0), RequestId(1), b"next", &mut out),
            Err(AppendCallRefusal::CompletionPending)
        );
        assert_eq!(
            owner.acknowledge_kernel_completion(
                NodeId(0),
                HostCallId(0),
                RequestId(0),
                AppendOutcome::Capacity
            ),
            Err(AppendCallRefusal::StaleRequest)
        );
        acknowledge(&mut owner, 0, outcome);
        acknowledge(&mut owner, 0, outcome);
        for i in 1..8 {
            let outcome = invoke(&mut owner, i, b"next");
            assert_eq!(outcome, AppendOutcome::Recorded(DurableRecordIndex(i)));
            acknowledge(&mut owner, i, outcome);
        }
        assert_eq!(owner.remaining_slots(), Some(0));
        assert_eq!(invoke(&mut owner, 8, b"next"), AppendOutcome::Capacity);
        assert_eq!(invoke(&mut owner, 8, b"next"), AppendOutcome::Capacity);
        acknowledge(&mut owner, 8, AppendOutcome::Capacity);
        assert_eq!(
            invoke(&mut owner, 9, b"still sampling"),
            AppendOutcome::Capacity
        );
        acknowledge(&mut owner, 9, AppendOutcome::Capacity);
        let (status, cost) = allocation_probe::observe(|| {
            owner.invoke(
                NodeId(0),
                HostCallId(0),
                RequestId(10),
                b"still sampling",
                &mut out,
            )
        });
        assert_eq!(decode(&out[..status.unwrap()]), AppendOutcome::Capacity);
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        acknowledge(&mut owner, 10, AppendOutcome::Capacity);
        assert_eq!(f.storage.programs.get(), 16);
        assert_eq!(
            owner.custody().unwrap().committed_index,
            Some(DurableRecordIndex(7))
        );
        assert!(owner.custody().unwrap().kernel_committed);
        owner.revoke().unwrap();
        assert_eq!(
            owner.invoke(
                NodeId(0),
                HostCallId(0),
                RequestId(10),
                b"still sampling",
                &mut out
            ),
            Err(AppendCallRefusal::Terminal)
        );
        let (ready, custody) = RecordAppendHostCall::retire_retained(&mut Some(owner)).unwrap();
        assert!(custody.unwrap().kernel_committed);
        let mut media = ready.into_provider();
        let summary = media.recover().unwrap();
        assert_eq!(summary.committed, 8);
        assert_eq!(summary.unpublished_or_damaged, 0);
        assert!(matches!(
            media.read(DurableRecordIndex(0)).unwrap(),
            RecordRead::Committed {
                payload: b"abc",
                ..
            }
        ));
    }
}
#[test]
fn invalid_or_cancelled_requests_do_not_program_and_uncertain_media_becomes_data() {
    let f = Fixture::new();
    let mut owner = f.bind(false).unwrap();
    let mut out = [0; APPEND_RESULT_BYTES as usize];
    assert_eq!(
        owner.invoke(NodeId(9), HostCallId(0), RequestId(0), b"x", &mut out),
        Err(AppendCallRefusal::WrongBinding)
    );
    assert_eq!(
        owner.invoke(NodeId(0), HostCallId(0), RequestId(0), &[0; 201], &mut out),
        Err(AppendCallRefusal::InvalidInput)
    );
    assert_eq!(f.storage.programs.get(), 0);
    f.storage.fail.set(true);
    assert_eq!(invoke(&mut owner, 0, b"x"), AppendOutcome::Uncertain);
    let programs = f.storage.programs.get();
    assert_eq!(invoke(&mut owner, 0, b"x"), AppendOutcome::Uncertain);
    acknowledge(&mut owner, 0, AppendOutcome::Uncertain);
    f.storage.fail.set(false);
    assert_eq!(
        invoke(&mut owner, 1, b"new sample"),
        AppendOutcome::RecoveryRequired
    );
    acknowledge(&mut owner, 1, AppendOutcome::RecoveryRequired);
    assert_eq!(f.storage.programs.get(), programs);
    let custody = owner.custody().unwrap();
    assert_eq!(custody.request, RequestId(0));
    assert_eq!(custody.committed_index, None);
    owner.revoke().unwrap();
    assert_eq!(
        RecordAppendHostCall::retire_retained(&mut Some(owner))
            .unwrap()
            .1,
        Some(custody)
    );
    let g = Fixture::new();
    let mut cancelled = g.bind(false).unwrap();
    cancelled.revoke().unwrap();
    assert_eq!(
        cancelled.invoke(NodeId(0), HostCallId(0), RequestId(0), b"x", &mut out),
        Err(AppendCallRefusal::Terminal)
    );
    assert_eq!(g.storage.programs.get(), 0);
}
#[test]
fn ready_rejects_foreign_original_authority_or_unrecovered_media() {
    let f = Fixture::new();
    let ready = f.ready();
    let id = ready.native_identity().clone();
    let provider =
        PreparedNorRecordResidence::prepare(f.storage.clone(), RecordResidenceId([3; 32]), 0, 8)
            .unwrap_or_else(|_| panic!("geometry"));
    assert!(matches!(
        unsafe { ReadyRecordAppender::new(id.clone(), provider, &grant(&scope()), 16) },
        Err(AppendCallRefusal::Media(
            RecordResidenceRefusal::RecoveryRequired
        ))
    ));
    let mut wrong = grant(&scope());
    wrong.boot_id = "foreign".into();
    assert!(matches!(
        unsafe { ReadyRecordAppender::new(id, ready.into_provider(), &wrong, 16) },
        Err(AppendCallRefusal::WrongBinding)
    ));
    assert_eq!(f.storage.programs.get(), 0);
}
#[test]
fn original_total_call_ceiling_is_separate_from_physical_write_capacity() {
    let f = Fixture::new();
    assert!(matches!(
        f.bind_with_limit(false, false, 17),
        Err(AppendCallRefusal::Possession)
    ));
    assert_eq!(f.storage.programs.get(), 0);
    let mut owner = f.bind_with_limit(false, false, 2).unwrap();
    for i in 0..2 {
        let outcome = invoke(&mut owner, i, b"x");
        acknowledge(&mut owner, i, outcome);
    }
    assert_eq!(owner.remaining_slots(), Some(6));
    assert!(matches!(
        owner.invoke(
            NodeId(0),
            HostCallId(0),
            RequestId(2),
            b"x",
            &mut [0; APPEND_RESULT_BYTES as usize]
        ),
        Err(AppendCallRefusal::Capability(_))
    ));
    assert_eq!(f.storage.programs.get(), 4);
}
#[test]
fn lost_final_media_response_preserves_uncertain_custody_without_repeating_effect() {
    let f = Fixture::new();
    let mut owner = f.bind(false).unwrap();
    f.storage.fail_commit_read.set(true);
    assert_eq!(
        invoke(&mut owner, 0, b"preserved"),
        AppendOutcome::Uncertain
    );
    assert_eq!(f.storage.programs.get(), 2);
    assert_eq!(
        invoke(&mut owner, 0, b"preserved"),
        AppendOutcome::Uncertain
    );
    acknowledge(&mut owner, 0, AppendOutcome::Uncertain);
    assert_eq!(
        invoke(&mut owner, 1, b"later sample"),
        AppendOutcome::RecoveryRequired
    );
    assert_eq!(owner.custody().unwrap().request, RequestId(0));
    owner.revoke().unwrap();
    let (ready, custody) = RecordAppendHostCall::retire_retained(&mut Some(owner)).unwrap();
    assert_eq!(custody.unwrap().committed_index, None);
    f.storage.fail_commit_read.set(false);
    let summary = ready.into_provider().recover().unwrap();
    assert_eq!(summary.committed, 1);
    assert_eq!(summary.next_index, 1);
    assert_eq!(summary.unpublished_or_damaged, 0);
    assert_eq!(f.storage.programs.get(), 2);
}
