use super::*;
use conduit_kernel::{HostCallOutcome, ValueRef};
fn back() -> JsonSnapshotBack {
    JsonSnapshotBack {
        input_bytes: 4096,
        output_bytes: 512,
        expected_reference: b"selected-ref-001".to_vec().into_boxed_slice(),
        write: true,
        pending: false,
        terminal: false,
    }
}
fn request(back: &mut JsonSnapshotBack, length: u32) -> StepOutcome {
    request_bytes(back, length, Some(b"null"))
}
fn request_bytes(back: &mut JsonSnapshotBack, length: u32, bytes: Option<&[u8]>) -> StepOutcome {
    let mut io = StepIo::test_frame(
        [Some(ValueRef {
            slot: 0,
            generation: 1,
            byte_len: length,
        })],
        [false],
        [Some(512)],
        None,
        4096,
    );
    back.step(&mut io, &StepInputBytes::test_frame([bytes], None))
}
fn completion(
    back: &mut JsonSnapshotBack,
    ready: bool,
    request: u32,
    length: u32,
    disposition: HostCallDisposition,
) -> StepOutcome {
    complete_bytes(
        back,
        ready,
        request,
        length,
        disposition,
        Some(b"selected-ref-001"),
    )
}
fn complete_bytes(
    back: &mut JsonSnapshotBack,
    ready: bool,
    request: u32,
    length: u32,
    disposition: HostCallDisposition,
    bytes: Option<&[u8]>,
) -> StepOutcome {
    let mut io = StepIo::test_frame(
        [None],
        [false],
        [ready.then_some(512)],
        Some((
            RequestId(request),
            HostCallOutcome {
                disposition,
                output: (disposition == HostCallDisposition::Completed).then(|| {
                    BoundedValueRef::new(
                        ValueRef {
                            slot: 1,
                            generation: 1,
                            byte_len: length,
                        },
                        length.max(512),
                    )
                    .unwrap()
                }),
                failure: matches!(
                    disposition,
                    HostCallDisposition::Failed | HostCallDisposition::Denied
                )
                .then_some(Failure {
                    code: FailureCode::HostCallDenied,
                    detail: 77,
                }),
            },
        )),
        4096,
    );
    back.step(&mut io, &StepInputBytes::test_frame([None], bytes))
}
#[test]
fn one_shot_waits_under_output_pressure_and_requires_original_completion() {
    let mut back = back();
    assert_eq!(request(&mut back, 4), StepOutcome::Progress);
    assert_eq!(
        completion(&mut back, false, 0, 16, HostCallDisposition::Completed),
        StepOutcome::Await
    );
    assert!(back.pending);
    assert!(!back.terminal);
    assert!(matches!(
        completion(&mut back, true, 1, 16, HostCallDisposition::Completed),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            ..
        })
    ));
    assert_eq!(
        completion(&mut back, true, 0, 16, HostCallDisposition::Completed),
        StepOutcome::Progress
    );
    assert!(back.terminal);
    assert!(!back.pending);
    assert_eq!(request(&mut back, 4), StepOutcome::Complete);
}
#[test]
fn bounds_failure_and_cancellation_never_emit_a_success_reference() {
    assert!(matches!(
        request(&mut back(), 4097),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            ..
        })
    ));
    for disposition in [
        HostCallDisposition::Denied,
        HostCallDisposition::Failed,
        HostCallDisposition::Cancelled,
    ] {
        let mut back = back();
        assert_eq!(request(&mut back, 4), StepOutcome::Progress);
        assert!(matches!(
            completion(&mut back, true, 0, 16, disposition),
            StepOutcome::Fail(_)
        ));
    }
    let mut oversized = back();
    request(&mut oversized, 4);
    assert!(matches!(
        completion(&mut oversized, true, 0, 513, HostCallDisposition::Completed),
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidInput,
            ..
        })
    ));
    <JsonSnapshotBack as StepBack<1>>::cancel(&mut oversized);
    assert_eq!(request(&mut oversized, 4), StepOutcome::Complete);
}
#[test]
fn binding_roles_are_independent_of_pool_sort_order() {
    let content = ResourceBinding {
        pool_id: "z/content".into(),
        class_id: JSON_SNAPSHOT_ACCESS_CLASS.into(),
        units: 1,
        compute: None,
        protected: None,
        content: None,
    };
    let memory = ResourceBinding {
        pool_id: "a/memory".into(),
        class_id: RUNTIME_MEMORY_RESOURCE_CLASS.into(),
        units: JSON_SNAPSHOT_PREPARATION_BYTES,
        compute: None,
        protected: None,
        content: None,
    };
    for resources in [
        [content.clone(), memory.clone()],
        [memory.clone(), content.clone()],
    ] {
        let (selected, preparation) = selected_bindings(&resources).unwrap();
        assert_eq!(selected, &content);
        assert_eq!(preparation, &memory);
    }
    assert!(selected_bindings(&[memory.clone(), memory]).is_err());
    assert!(selected_bindings(&[content]).is_err());
}
#[test]
fn exact_factory_lookup_is_explicit_without_ambient_inventory_changes() {
    for id in [
        JSON_SNAPSHOT_PUBLISH_IMPLEMENTATION,
        JSON_SNAPSHOT_READ_IMPLEMENTATION,
    ] {
        assert_eq!(
            super::super::catalog::factory(&id.into())
                .unwrap()
                .implementation_id,
            id
        );
    }
    assert!(super::super::catalog::factory(&"std/unknown-json-snapshot@1".into()).is_none());
}
#[test]
fn foreign_or_missing_publication_completion_never_commits_even_under_pressure() {
    for bytes in [
        Some(b"foreign-ref-0001".as_slice()),
        Some(b"malformed".as_slice()),
        None,
    ] {
        let mut back = back();
        request(&mut back, 4);
        let probe = crate::allocation_probe::begin();
        assert!(matches!(
            complete_bytes(
                &mut back,
                false,
                0,
                16,
                HostCallDisposition::Completed,
                bytes
            ),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                ..
            })
        ));
        assert!(back.pending && !back.terminal);
        assert_eq!(probe.finish(), 0);
    }
}

#[test]
fn read_requires_exact_reference_and_valid_json_without_growth() {
    for reference in [
        Some(b"foreign-ref-0001".as_slice()),
        Some(b"malformed".as_slice()),
        None,
    ] {
        let mut back = back();
        back.write = false;
        let probe = crate::allocation_probe::begin();
        assert!(matches!(
            request_bytes(&mut back, 16, reference),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                ..
            })
        ));
        assert!(!back.pending);
        assert_eq!(probe.finish(), 0);
    }
    for json in [
        Some(b"not json".as_slice()),
        Some(b"{]".as_slice()),
        Some(b"\xff".as_slice()),
        None,
    ] {
        let mut back = back();
        back.write = false;
        assert_eq!(
            request_bytes(&mut back, 16, Some(b"selected-ref-001")),
            StepOutcome::Progress
        );
        let probe = crate::allocation_probe::begin();
        assert!(matches!(
            complete_bytes(
                &mut back,
                true,
                0,
                json.map_or(4, |b| b.len() as u32),
                HostCallDisposition::Completed,
                json
            ),
            StepOutcome::Fail(_)
        ));
        assert_eq!(probe.finish(), 0);
    }
    let mut back = back();
    back.write = false;
    let heap = back.prepared_heap_bytes();
    let probe = crate::allocation_probe::begin();
    assert_eq!(
        request_bytes(&mut back, 16, Some(b"selected-ref-001")),
        StepOutcome::Progress
    );
    assert_eq!(
        complete_bytes(
            &mut back,
            false,
            0,
            4,
            HostCallDisposition::Completed,
            Some(b"null")
        ),
        StepOutcome::Await
    );
    assert!(back.pending && !back.terminal);
    assert_eq!(
        complete_bytes(
            &mut back,
            true,
            0,
            4,
            HostCallDisposition::Completed,
            Some(b"null")
        ),
        StepOutcome::Progress
    );
    assert_eq!(back.prepared_heap_bytes(), heap);
    assert_eq!(probe.finish(), 0);
}

#[test]
fn publish_invalid_json_refuses_before_request_and_backing_is_accounted() {
    let mut back = back();
    let probe = crate::allocation_probe::begin();
    assert!(matches!(
        request_bytes(&mut back, 4, Some(b"nope")),
        StepOutcome::Fail(_)
    ));
    assert!(!back.pending);
    assert_eq!(probe.finish(), 0);
    let expected = back.expected_reference.len();
    assert_eq!(
        InstalledBack::JsonSnapshot(back).allocation_capacity(),
        expected
    );
}

#[test]
fn early_json_validation_preserves_distinct_provider_refusal_categories() {
    let too_deep = b"[[[[[[[[[null]]]]]]]]]";
    let too_large = [b' '; 4097];
    for (bytes, detail) in [
        (b"{]".as_slice(), 1001),
        (b"\xff".as_slice(), 1002),
        (too_deep.as_slice(), 1003),
        (too_large.as_slice(), 1010),
    ] {
        // Widen only this fixture's admission so the JSON extent category is
        // observed independently of an earlier typed Port extent refusal.
        let mut publish = back();
        publish.input_bytes = 8192;
        let mut read = back();
        read.write = false;
        read.output_bytes = 8192;
        request_bytes(&mut read, 16, Some(b"selected-ref-001"));
        let probe = crate::allocation_probe::begin();
        assert_eq!(
            request_bytes(&mut publish, bytes.len() as u32, Some(bytes)),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail
            })
        );
        assert_eq!(
            complete_bytes(
                &mut read,
                true,
                0,
                bytes.len() as u32,
                HostCallDisposition::Completed,
                Some(bytes)
            ),
            StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail
            })
        );
        assert_eq!(probe.finish(), 0);
    }
}
