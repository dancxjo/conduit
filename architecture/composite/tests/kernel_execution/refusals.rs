//! Runtime failures retain exact typed causes without Play allocation.
use super::*;

#[test]
fn child_refusal_is_distinct_and_occurs_before_play() {
    assert!(matches!(
        KernelCompositeHost::prepare(definition(), &KernelOperationRegistry::new()),
        Err(KernelCompositeError::ChildRefused { .. })
    ));
}

#[test]
fn child_failure_is_a_machine_readable_kernel_execution_terminal() {
    let mut failed = KernelCompositeHost::prepare(definition(), &failing_registry()).unwrap();
    failed.start().unwrap();
    let allocations = allocations_during(|| {
        assert_eq!(
            failed.step(),
            Err(KernelCompositeError::Execution {
                child: 0,
                reason: conduit_composite::ChildExecutionError::Scheduler(
                    conduit_kernel::scheduler::SchedulerError::BackFailed(
                        conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidLifecycle,
                            detail: 17,
                        }
                    ),
                ),
            }),
        );
        assert_eq!(failed.child_identity(0).unwrap().as_str(), "first-child");
        assert!(failed.child_identity(2).is_none());
    });
    assert_eq!(allocations, 0, "kernel execution refusal allocated");
}

#[test]
fn stale_child_identity_refuses_before_any_kernel_is_started() {
    let mut stale = definition();
    stale.boundary.input_fronts[0].internal_child = HostId::from("stale-child");
    assert!(matches!(
        KernelCompositeHost::prepare(stale, &registry()),
        Err(KernelCompositeError::StaleChild(_))
    ));
}

#[test]
fn malformed_boundary_binding_and_value_kind_refuse_distinctly() {
    let mut malformed = definition();
    malformed.boundary.input_fronts[0].internal_port_id = conduit_core::port_id("missing");
    assert!(matches!(
        KernelCompositeHost::prepare(malformed, &registry()),
        Err(KernelCompositeError::InvalidBoundary(_))
    ));

    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    assert_eq!(host.step(), Err(KernelCompositeError::InvalidLifecycle));
    host.start().unwrap();
    let input = conduit_core::port_id("input");
    let unknown = conduit_core::port_id("unknown");
    let wrong = ValuePayload {
        value_kind: kind_id("value/wrong"),
        encoded: vec![1],
    };
    let mut undersized = ValuePayload {
        value_kind: wrong.value_kind.clone(),
        encoded: Vec::new(),
    };
    let allocations = allocations_during(|| {
        assert_eq!(
            host.admit_input(&input, 0, &wrong),
            Err(KernelCompositeError::Execution {
                child: 0,
                reason: conduit_composite::ChildExecutionError::ValueKindMismatch,
            }),
        );
        assert_eq!(
            host.admit_input(&unknown, 0, &wrong),
            Err(KernelCompositeError::UnknownFront)
        );
        assert_eq!(
            host.close_input_abnormal(&input, &wrong),
            Err(KernelCompositeError::Execution {
                child: 0,
                reason: conduit_composite::ChildExecutionError::ValueKindMismatch,
            })
        );
        // No output is offered yet; asking for a wrong buffer cannot create one.
        assert_eq!(
            host.output_into(&unknown, &mut undersized),
            Err(KernelCompositeError::UnknownFront)
        );
    });
    assert_eq!(allocations, 0, "boundary refusal allocated");
    let output = conduit_core::port_id("output");
    let bytes = value(b"ok");
    let mut exact = value(&[]);
    exact.encoded.reserve(16);
    let mut too_small = value(&[]);
    host.admit_input(&input, 0, &bytes).unwrap();
    for _ in 0..4 {
        host.step().unwrap();
    }
    let allocations = allocations_during(|| {
        for buffer in [&mut undersized, &mut too_small] {
            assert_eq!(
                host.output_into(&output, buffer),
                Err(KernelCompositeError::Execution {
                    child: 1,
                    reason: conduit_composite::ChildExecutionError::BufferContractMismatch,
                })
            );
        }
        assert_eq!(host.output_into(&output, &mut exact), Ok(Some(0)));
        assert_eq!(exact.encoded, b"ok");
        host.complete_output(&output, 0).unwrap();
        assert_eq!(host.output_into(&output, &mut exact), Ok(None));
    });
    assert_eq!(allocations, 0, "output refusal or recovery allocated");
}

#[test]
fn cancellation_is_terminal_and_rejects_late_kernel_work() {
    let mut host = KernelCompositeHost::prepare(definition(), &registry()).unwrap();
    let input_port = conduit_core::port_id("input");
    let late = value(b"late");
    host.start().unwrap();
    let allocations = allocations_during(|| {
        host.cancel().unwrap();
        assert_eq!(host.step().unwrap(), KernelCompositeStatus::Cancelled);
        assert!(matches!(
            host.admit_input(&input_port, 0, &late),
            Err(KernelCompositeError::InvalidLifecycle)
        ));
    });
    assert_eq!(allocations, 0, "cancellation allocated {allocations} times");
}
