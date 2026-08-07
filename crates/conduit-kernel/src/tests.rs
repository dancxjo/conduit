use super::{
    BoundedValueRef, CordId, EvidenceError, EvidenceSink, FixedEvidenceLog,
    FixedHostOperationBindings, FixedRoutes, FixedValueStore, HostOperationBinding,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, KernelEvent, KernelEventKind,
    NodeId, Operation, OperationAction, OperationInput, PortId, RequestId, RouteRange, RouteTarget,
    StorageError, ValueStorage,
};

#[test]
fn port_aware_actions_and_inputs_preserve_exact_identity() {
    struct Echo {
        input: PortId,
        output: PortId,
    }

    impl Operation for Echo {
        fn start(&mut self) -> OperationAction {
            OperationAction::Await
        }

        fn resume(&mut self, input: OperationInput) -> OperationAction {
            match input {
                OperationInput::Value { port, value } if port == self.input => {
                    OperationAction::Emit {
                        port: self.output,
                        value,
                    }
                }
                OperationInput::Closed { port } if port == self.input => OperationAction::Complete,
                _ => OperationAction::Fail(super::Failure {
                    code: super::FailureCode::InvalidPort,
                    detail: 0,
                }),
            }
        }
    }

    let mut operation = Echo {
        input: PortId(3),
        output: PortId(7),
    };
    let value = super::ValueRef {
        slot: 1,
        generation: 2,
        byte_len: 4,
    };
    assert_eq!(operation.start(), OperationAction::Await);
    assert_eq!(
        operation.resume(OperationInput::Value {
            port: PortId(3),
            value
        }),
        OperationAction::Emit {
            port: PortId(7),
            value
        }
    );
    assert_eq!(
        operation.resume(OperationInput::Closed { port: PortId(3) }),
        OperationAction::Complete
    );
}

#[test]
fn prebound_routes_never_broadcast_between_output_ports() {
    let mut routes = FixedRoutes::<4, 3>::new(2);
    routes
        .install(
            NodeId(0),
            PortId(0),
            RouteRange { start: 0, len: 2 },
            &[
                RouteTarget {
                    cord: CordId(0),
                    sink: crate::CordEndpoint::local(NodeId(1), PortId(0)),
                },
                RouteTarget {
                    cord: CordId(1),
                    sink: crate::CordEndpoint::local(NodeId(2), PortId(0)),
                },
            ],
        )
        .unwrap();
    routes
        .install(
            NodeId(0),
            PortId(1),
            RouteRange { start: 2, len: 1 },
            &[RouteTarget {
                cord: CordId(2),
                sink: crate::CordEndpoint::local(NodeId(3), PortId(4)),
            }],
        )
        .unwrap();
    routes.seal().unwrap();

    let mut left = routes.route(NodeId(0), PortId(0)).unwrap();
    assert_eq!(left.next().unwrap().cord, CordId(0));
    assert_eq!(left.next().unwrap().cord, CordId(1));
    assert_eq!(left.next(), None);
    let mut right = routes.route(NodeId(0), PortId(1)).unwrap();
    let right = right.next().unwrap();
    assert_eq!(right.cord, CordId(2));
    assert_eq!(right.sink, crate::CordEndpoint::local(NodeId(3), PortId(4)));
}

#[test]
fn fixed_value_store_enforces_items_bytes_generation_and_fanout_references() {
    let mut store = FixedValueStore::<2, 8>::new(10).unwrap();
    let first = store.store(b"abcd").unwrap();
    let second = store.store(b"123456").unwrap();
    assert_eq!(store.used_items(), 2);
    assert_eq!(store.used_bytes(), 10);
    assert_eq!(store.store(b"x"), Err(StorageError::ByteCapacityExceeded));
    assert_eq!(store.get(first).unwrap(), b"abcd");

    store.retain(first).unwrap();
    store.release(first).unwrap();
    assert_eq!(store.get(first).unwrap(), b"abcd");
    store.release(first).unwrap();
    assert_eq!(store.get(first), Err(StorageError::StaleReference));
    assert_eq!(store.used_items(), 1);
    assert_eq!(store.used_bytes(), 6);

    let replacement = store.store(b"xy").unwrap();
    assert_eq!(replacement.slot, first.slot);
    assert_ne!(replacement.generation, first.generation);
    assert_eq!(store.get(second).unwrap(), b"123456");
}

#[test]
fn host_operation_completion_is_correlated_and_byte_admitted() {
    let value = super::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 4,
    };
    let bounded = BoundedValueRef::new(value, 4).unwrap();
    let action = OperationAction::RequestHostOperation {
        request: RequestId(9),
        operation: HostOperationId(2),
        input: bounded,
    };
    assert!(matches!(
        action,
        OperationAction::RequestHostOperation {
            request: RequestId(9),
            operation: HostOperationId(2),
            ..
        }
    ));
    let input = OperationInput::HostOperationCompleted {
        request: RequestId(9),
        outcome: HostOperationOutcome {
            disposition: HostOperationDisposition::Completed,
            output: Some(bounded),
            failure: None,
        },
    };
    assert!(matches!(
        input,
        OperationInput::HostOperationCompleted {
            request: RequestId(9),
            ..
        }
    ));
    assert!(BoundedValueRef::new(value, 3).is_err());
}

#[test]
fn only_plan_admitted_host_operations_cross_the_boundary() {
    let value = super::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 4,
    };
    let mut bindings = FixedHostOperationBindings::<4>::new(2);
    bindings
        .install(
            NodeId(1),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 4,
                maximum_output_bytes: 8,
            },
        )
        .unwrap();
    bindings.seal().unwrap();
    let action = OperationAction::RequestHostOperation {
        request: RequestId(7),
        operation: HostOperationId(0),
        input: BoundedValueRef::new(value, 4).unwrap(),
    };
    assert_eq!(
        bindings
            .admit(NodeId(1), action)
            .unwrap()
            .maximum_output_bytes,
        8
    );
    assert!(bindings.admit(NodeId(0), action).is_err());
}

#[test]
fn admitted_sink_host_operation_may_have_no_output_payload() {
    let mut bindings = FixedHostOperationBindings::<1>::new(1);
    bindings
        .install(
            NodeId(0),
            HostOperationBinding {
                operation: HostOperationId(0),
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )
        .unwrap();
    bindings.seal().unwrap();

    let action = OperationAction::RequestHostOperation {
        request: RequestId(1),
        operation: HostOperationId(0),
        input: BoundedValueRef::new(
            super::ValueRef {
                slot: 0,
                generation: 1,
                byte_len: 4,
            },
            4,
        )
        .unwrap(),
    };
    assert_eq!(
        bindings
            .admit(NodeId(0), action)
            .unwrap()
            .maximum_output_bytes,
        0
    );
}

#[test]
fn fixed_evidence_has_independent_item_and_byte_budgets() {
    let charge = u32::try_from(core::mem::size_of::<KernelEvent>()).unwrap();
    let mut log = FixedEvidenceLog::<3>::new(charge * 2).unwrap();
    log.record(
        NodeId(0),
        Some(PortId(1)),
        None,
        KernelEventKind::ValueRouted,
    )
    .unwrap();
    log.record(
        NodeId(1),
        None,
        Some(RequestId(2)),
        KernelEventKind::HostOperationCompleted,
    )
    .unwrap();
    assert_eq!(
        log.record(NodeId(2), None, None, KernelEventKind::OperationCompleted),
        Err(EvidenceError::ByteCapacityExceeded)
    );
    let mut events = log.events();
    assert_eq!(events.next().unwrap().sequence, 0);
    assert_eq!(events.next().unwrap().sequence, 1);
    assert_eq!(events.next(), None);
}

#[cfg(feature = "alloc")]
#[test]
fn hosted_and_fixed_value_profiles_produce_the_same_storage_vector() {
    use super::{HostedEvidenceLog, HostedValueStore};

    fn vector(storage: &mut impl ValueStorage) -> (u16, u32, [u8; 3]) {
        let value = storage.store(b"abc").unwrap();
        let mut bytes = [0; 3];
        bytes.copy_from_slice(storage.get(value).unwrap());
        storage.retain(value).unwrap();
        storage.release(value).unwrap();
        (storage.used_items(), storage.used_bytes(), bytes)
    }

    let mut fixed = FixedValueStore::<4, 8>::new(16).unwrap();
    let mut hosted = HostedValueStore::new(4, 8, 16).unwrap();
    assert_eq!(vector(&mut fixed), vector(&mut hosted));

    fn evidence_vector(sink: &mut impl EvidenceSink) -> (u16, u32, KernelEvent) {
        let event = sink
            .record(
                NodeId(1),
                Some(PortId(2)),
                Some(RequestId(3)),
                KernelEventKind::HostOperationCompleted,
            )
            .unwrap();
        (sink.len(), sink.used_bytes(), event)
    }
    let charge = u32::try_from(core::mem::size_of::<KernelEvent>()).unwrap();
    let mut fixed_evidence = FixedEvidenceLog::<2>::new(charge * 2).unwrap();
    let mut hosted_evidence = HostedEvidenceLog::new(2, charge * 2).unwrap();
    assert_eq!(
        evidence_vector(&mut fixed_evidence),
        evidence_vector(&mut hosted_evidence)
    );
}
