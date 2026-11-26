#![cfg(feature = "kernel-step")]
//! Direct Back conformance with synthetic reference bytes; no storage authority.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome, RequestId,
    ValueRef,
};
use conduit_resource_snapshot::{SnapshotContentValidator, SnapshotPublicationBack};
struct Bytes;
impl SnapshotContentValidator for Bytes {
    type Metadata = u8;
    fn validate(&mut self, bytes: &[u8]) -> Result<u8, Failure> {
        if bytes[0] == 0 {
            return Err(Failure {
                code: FailureCode::InvalidInput,
                detail: 17,
            });
        }
        Ok(bytes[0])
    }
}
fn request(back: &mut SnapshotPublicationBack<Bytes>, bytes: &[u8]) -> StepOutcome {
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: bytes.len() as u32,
        })],
        [false],
        [Some(16)],
        None,
        16,
    );
    back.step(&mut io, &StepInputBytes::test_frame([Some(bytes)], None))
}
fn completion(
    back: &mut SnapshotPublicationBack<Bytes>,
    ready: bool,
    request: u32,
    bytes: &[u8],
) -> StepOutcome {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [ready.then_some(16)],
        Some((
            RequestId(request),
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        ValueRef {
                            slot: 1,
                            generation: 1,
                            byte_len: bytes.len() as u32,
                        },
                        16,
                    )
                    .unwrap(),
                ),
                failure: None,
            },
        )),
        16,
    );
    back.step(&mut io, &StepInputBytes::test_frame([None], Some(bytes)))
}
fn commit(back: &mut SnapshotPublicationBack<Bytes>) {
    <SnapshotPublicationBack<Bytes> as StepBack<1>>::step_committed(back);
}
#[test]
fn exact_bytes_become_visible_only_after_output_commit_and_survive_later_cancel() {
    let (mut back, observer) =
        SnapshotPublicationBack::prepare(Bytes, 3, 16, b"ref".to_vec(), "test/bytes@1").unwrap();
    assert_eq!(request(&mut back, b"abc"), StepOutcome::Progress);
    assert_eq!(observer.committed(), None);
    commit(&mut back);
    assert_eq!(completion(&mut back, false, 0, b"ref"), StepOutcome::Await);
    assert!(!observer.committed_content_matches(b"abc"));
    assert_eq!(
        completion(&mut back, true, 0, b"ref"),
        StepOutcome::Progress
    );
    assert_eq!(observer.committed(), None);
    commit(&mut back);
    assert_eq!(observer.committed(), Some(b'a'));
    assert!(observer.committed_content_matches(b"abc"));
    assert!(!observer.committed_content_matches(b"abd"));
    <SnapshotPublicationBack<Bytes> as StepBack<1>>::cancel(&mut back);
    assert!(observer.committed_content_matches(b"abc"));
}
#[test]
fn wrong_completion_and_cancel_never_publish() {
    for wrong in [true, false] {
        let (mut back, observer) =
            SnapshotPublicationBack::prepare(Bytes, 3, 16, b"ref".to_vec(), "test/bytes@1")
                .unwrap();
        assert_eq!(request(&mut back, b"abc"), StepOutcome::Progress);
        commit(&mut back);
        if wrong {
            assert!(matches!(
                completion(&mut back, true, 1, b"ref"),
                StepOutcome::Fail(_)
            ));
        } else {
            <SnapshotPublicationBack<Bytes> as StepBack<1>>::cancel(&mut back);
            assert!(matches!(
                completion(&mut back, true, 0, b"ref"),
                StepOutcome::Fail(_)
            ));
        }
        assert_eq!(observer.committed(), None);
    }
}
#[test]
fn preparation_bounds_and_domain_refusals_do_not_publish() {
    assert!(
        SnapshotPublicationBack::prepare(Bytes, 17, 16, b"ref".to_vec(), "test/bytes@1").is_err()
    );
    let (mut back, observer) =
        SnapshotPublicationBack::prepare(Bytes, 3, 16, b"ref".to_vec(), "test/bytes@1").unwrap();
    assert_eq!(
        request(&mut back, &[0, 1, 2]),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            detail: 17
        })
    );
    assert_eq!(observer.committed(), None);
}
