//! Actual opaque operation + NOR provider tests; selection binder is separate.
use super::*;
#[path = "tests/fixture.rs"]
mod fixture;
use fixture::*;
fn invoke(
    owner: &mut RecordServiceHostCall<Storage>,
    read: bool,
    request: u32,
    input: &[u8],
) -> alloc::vec::Vec<u8> {
    let mut output = [0; crate::read_result::READ_RESULT_BYTES];
    let length = owner
        .invoke(
            NodeId(3),
            HostCallId(if read { 0 } else { 1 }),
            RequestId(request),
            input,
            &mut output,
        )
        .unwrap();
    output[..length].to_vec()
}
fn acknowledge(
    owner: &mut RecordServiceHostCall<Storage>,
    read: bool,
    request: u32,
    outcome: &[u8],
) {
    owner
        .acknowledge_kernel_completion(
            NodeId(3),
            HostCallId(if read { 0 } else { 1 }),
            RequestId(request),
            outcome,
        )
        .unwrap();
}
#[test]
fn append_read_share_one_media_owner_and_global_request_cache() {
    let (mut owner, stats) = fixture(false);
    let append = invoke(&mut owner, false, 0, b"actual record");
    assert_eq!(
        crate::append_result::decode(&append).unwrap(),
        crate::append_result::DecodedAppendResult::Recorded(0)
    );
    let before = (stats.reads.get(), stats.programs.get());
    assert_eq!(invoke(&mut owner, false, 0, b"actual record"), append);
    assert_eq!((stats.reads.get(), stats.programs.get()), before);
    let mut output = [0; crate::read_result::READ_RESULT_BYTES];
    assert_eq!(
        owner.invoke(
            NodeId(3),
            HostCallId(0),
            RequestId(0),
            &c::encode_count(0),
            &mut output
        ),
        Err(ServiceRefusal::StaleRequest)
    );
    assert_eq!(
        owner.invoke(
            NodeId(3),
            HostCallId(0),
            RequestId(1),
            &c::encode_count(0),
            &mut output
        ),
        Err(ServiceRefusal::CompletionPending)
    );
    acknowledge(&mut owner, false, 0, &append);
    let read = invoke(&mut owner, true, 1, &c::encode_count(0));
    assert_eq!(stats.reads.get() - before.0, 2);
    assert_eq!(
        crate::read_result::decode(&read).unwrap(),
        ReadResult::Recorded {
            index: 0,
            captured_end: 1,
            record: b"actual record"
        }
    );
    let reads = stats.reads.get();
    assert_eq!(invoke(&mut owner, true, 1, &c::encode_count(0)), read);
    assert_eq!(stats.reads.get(), reads);
    assert_eq!(
        owner.acknowledge_kernel_completion(NodeId(3), HostCallId(1), RequestId(1), &read),
        Err(ServiceRefusal::StaleRequest)
    );
    acknowledge(&mut owner, true, 1, &read);
    let next = invoke(&mut owner, false, 2, b"second");
    acknowledge(&mut owner, false, 2, &next);
    let read = invoke(&mut owner, true, 3, &c::encode_count(0));
    assert_eq!(
        crate::read_result::decode(&read).unwrap(),
        ReadResult::Recorded {
            index: 0,
            captured_end: 2,
            record: b"actual record"
        }
    );
}
#[test]
fn end_and_invalid_index_are_finite_and_output_pressure_performs_no_media() {
    let (mut owner, stats) = fixture(false);
    let mut output = [0; crate::read_result::READ_RESULT_BYTES];
    let reads = stats.reads.get();
    assert_eq!(
        owner.invoke(
            NodeId(3),
            HostCallId(0),
            RequestId(0),
            &c::encode_count(0),
            &mut []
        ),
        Err(ServiceRefusal::OutputPressure)
    );
    assert_eq!(
        owner.invoke(
            NodeId(3),
            HostCallId(0),
            RequestId(0),
            &c::encode_count(1),
            &mut output
        ),
        Err(ServiceRefusal::InvalidInput)
    );
    assert_eq!(
        owner.invoke(
            NodeId(3),
            HostCallId(0),
            RequestId(0),
            &c::encode_count(u64::from(u32::MAX) + 1),
            &mut output
        ),
        Err(ServiceRefusal::InvalidInput)
    );
    assert_eq!(stats.reads.get(), reads);
    let end = invoke(&mut owner, true, 0, &c::encode_count(0));
    assert_eq!(
        crate::read_result::decode(&end).unwrap(),
        ReadResult::End(0)
    );
    assert_eq!(stats.reads.get(), reads);
    assert_eq!(owner.inspection(1).unwrap().completed_operations, 1);
    assert_eq!(owner.inspection(0).unwrap().completed_operations, 0);
}
#[test]
fn uncertain_append_custody_survives_revoked_idle_retirement() {
    let (mut owner, stats) = fixture(true);
    let outcome = invoke(&mut owner, false, 0, b"uncertain");
    assert_eq!(
        crate::append_result::decode(&outcome).unwrap(),
        crate::append_result::DecodedAppendResult::Uncertain
    );
    assert!(!owner.custody().unwrap().kernel_committed);
    let writes = stats.programs.get();
    assert_eq!(invoke(&mut owner, false, 0, b"uncertain"), outcome);
    assert_eq!(stats.programs.get(), writes);
    owner.revoke().unwrap();
    let mut owner = Some(owner);
    let (ready, custody) = RecordServiceHostCall::retire_retained(&mut owner).unwrap();
    assert!(owner.is_none());
    assert_eq!(custody.unwrap().committed_index, None);
    assert_eq!(ready.provider.recovered(), false);
    assert_eq!(ready.provider.residence_id(), RecordResidenceId([3; 32]));
}

#[path = "tests/rich.rs"]
mod rich;

#[test]
fn report_complete_service_inline_and_retained_heap_charges() {
    let (owner, _) = fixture(false);
    println!(
        "native service: owner_inline={}, ready_inline={}, storage_inline={}, provider_inline={}, ready_retained_heap={}, complete_retained_heap={}",
        core::mem::size_of::<RecordServiceHostCall<Storage>>(),
        core::mem::size_of::<ReadyRecordService<Storage>>(),
        core::mem::size_of::<Storage>(),
        core::mem::size_of::<PreparedNorRecordResidence<Storage>>(),
        owner.ready.retained_backing_bytes().unwrap(),
        owner.retained_backing_bytes().unwrap()
    );
    assert!(
        owner.retained_backing_bytes().unwrap() > owner.ready.retained_backing_bytes().unwrap()
    );
}

use crate::append::tests::allocation_probe;

#[test]
fn prepared_append_read_and_exact_retries_allocate_nothing() {
    let (mut owner, _) = fixture(false);
    let record = [42; 200];
    let index = c::encode_count(0);
    let mut output = [0; crate::read_result::READ_RESULT_BYTES];
    let (_, observation) = allocation_probe::observe(|| {
        let len = owner
            .invoke(NodeId(3), HostCallId(1), RequestId(0), &record, &mut output)
            .unwrap();
        assert_eq!(
            owner
                .invoke(NodeId(3), HostCallId(1), RequestId(0), &record, &mut output)
                .unwrap(),
            len
        );
        owner
            .acknowledge_kernel_completion(NodeId(3), HostCallId(1), RequestId(0), &output[..len])
            .unwrap();
        let len = owner
            .invoke(NodeId(3), HostCallId(0), RequestId(1), &index, &mut output)
            .unwrap();
        assert_eq!(
            owner
                .invoke(NodeId(3), HostCallId(0), RequestId(1), &index, &mut output)
                .unwrap(),
            len
        );
        assert_eq!(
            crate::read_result::decode(&output[..len]).unwrap(),
            ReadResult::Recorded {
                index: 0,
                captured_end: 1,
                record: &record
            }
        );
        owner
            .acknowledge_kernel_completion(NodeId(3), HostCallId(0), RequestId(1), &output[..len])
            .unwrap();
    });
    assert_eq!(observation.allocation_attempts, 0, "{observation:?}");
    assert_eq!(observation.reallocations, 0, "{observation:?}");
}
