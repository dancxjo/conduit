//! Actual numeric-kernel requests through the Root ledger, no protected guest claim.
use super::*;
use conduit_kernel::scheduler::{CordCapacity, CordSpec};

fn admission() -> Admission {
    let mut admission = Admission::new(2, 2);
    admission.backs[0] = BackKind::Document;
    admission.backs[1] = BackKind::Editor;
    for node in &mut admission.nodes[..2] {
        node.maximum_step_fuel = 16;
    }
    admission.nodes[0].input_cords[0] = Some(CordId(1));
    admission.nodes[1].input_cords[0] = Some(CordId(0));
    for (index, source, sink, bytes) in [(0, 0, 1, 264), (1, 1, 0, 272)] {
        admission.cords[index] = CordSpec::local(
            CordId(index as u16),
            (NodeId(source), PortId(0)),
            (NodeId(sink), PortId(0)),
            CordCapacity {
                slot_start: index as u16,
                item_capacity: 1,
                byte_capacity: bytes,
                pressure_policy: Default::default(),
            },
        )
        .with_maximum_value_bytes(bytes);
        admission
            .install_route(
                NodeId(source),
                PortId(0),
                RouteRange {
                    start: index as u16,
                    len: 1,
                },
                &[RouteTarget {
                    cord: CordId(index as u16),
                    sink: CordEndpoint::local(NodeId(sink), PortId(0)),
                }],
            )
            .unwrap();
    }
    admission.bindings[0] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: 272,
        maximum_output_bytes: 264,
    });
    admission.bindings[1] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: 264,
        maximum_output_bytes: 272,
    });
    admission
}
fn next(kernel: &mut Scheduler) -> HostCallRequest {
    (0..16)
        .find_map(|_| {
            kernel.step().unwrap();
            kernel.next_host_request()
        })
        .expect("bounded kernel steps reach the exact Host Call")
}
fn complete(kernel: &mut Scheduler, ledger: &mut Ledger, request: HostCallRequest, bytes: &[u8]) {
    let value = kernel.store_host_value(bytes).unwrap();
    ledger
        .publish(request.node.0 as usize, Some(value), bytes)
        .unwrap();
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(BoundedValueRef::new(value, bytes.len() as u32).unwrap()),
                failure: None,
            },
        )
        .unwrap();
}

#[test]
fn exact_initial_document_has_one_root_loan_then_real_editor_completions_supply_successors() {
    let admission = admission();
    let mut ledger = Ledger::new(&admission).unwrap();
    let mut kernel = admission.build().unwrap();
    let initial = next(&mut kernel);
    assert_eq!(initial.node, NodeId(1));
    assert_eq!(kernel.host_value(initial.input.value).unwrap(), &[0; 8]);
    // Failed byte, revision, size, and route claims cannot consume the startup loan.
    let mut revision_one = [0; 8];
    revision_one[0] = 1;
    assert!(ledger.accept(initial, &revision_one).is_err());
    assert!(ledger.accept(initial, &[]).is_err());
    assert!(ledger.accept(initial, &[0; 9]).is_err());
    let mut wrong_target = initial;
    wrong_target.node = NodeId(0);
    assert!(ledger.accept(wrong_target, &[0; 8]).is_err());
    ledger.accept(initial, &[0; 8]).unwrap();
    assert!(ledger.accept(initial, &[0; 8]).is_err());
    let mut forged = initial;
    forged.input.value = kernel.store_host_value(&[0; 8]).unwrap();
    assert!(
        ledger.accept(forged, &[0; 8]).is_err(),
        "a different guest reference creates no second loan"
    );
    // Complete the Editor's real pending call with a canonical empty insertion.
    complete(&mut kernel, &mut ledger, initial, &[0; 16]);
    let edit = next(&mut kernel);
    assert_eq!(edit.node, NodeId(0));
    assert_eq!(edit.request, RequestId(0));
    ledger
        .accept(edit, kernel.host_value(edit.input.value).unwrap())
        .unwrap();
    assert!(ledger.accept(edit, &[0; 16]).is_err());
    // The protected document owner returns the next canonical revision. This is
    // a Root-issued completion loan, never another implicit initialization.
    complete(&mut kernel, &mut ledger, edit, &revision_one);
    let successor = next(&mut kernel);
    assert_eq!(successor.node, NodeId(1));
    assert_eq!(successor.request, RequestId(1));
    assert!(ledger.accept(successor, &[0; 8]).is_err());
    let mut wrong_reference = successor;
    wrong_reference.input.value = forged.input.value;
    assert!(
        ledger.accept(wrong_reference, &revision_one).is_err(),
        "Root-completed successors bind both exact bytes and exact reference"
    );
    ledger.accept(successor, &revision_one).unwrap();
    assert!(ledger.accept(successor, &revision_one).is_err());
    ledger.clear();
    assert!(ledger.accept(initial, &[0; 8]).is_err());
    kernel.cancel().unwrap();
}

#[test]
fn editor_without_document_back_does_not_receive_an_initial_document_grant() {
    let mut admission = admission();
    admission.backs[0] = BackKind::Upper;
    let mut ledger = Ledger::new(&admission).unwrap();
    // Obtain a real request from the canonical configuration; changing only its
    // guest-issued identities cannot manufacture a missing reviewed Root loan.
    let mut kernel = self::admission().build().unwrap();
    let request = next(&mut kernel);
    assert!(ledger.accept(request, &[0; 8]).is_err());
    kernel.cancel().unwrap();
}

#[test]
fn canonical_fore_receipts_bind_exact_bytes_route_and_sequence_without_new_grants() {
    use conduit_kernel::scheduler::CordCapacity;
    let mut admission = Admission::new(1, 1);
    admission.backs[0] = BackKind::Face;
    admission.nodes[0].input_cords[0] = Some(CordId(0));
    admission.cords[0] = conduit_kernel::scheduler::CordSpec::remote_ingress(
        CordId(0),
        RemoteEndpointId(4),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: FACE_VALUE_BYTES as u32,
            pressure_policy: Default::default(),
        },
    )
    .with_maximum_value_bytes(FACE_VALUE_BYTES as u32);
    assert!(
        Ledger::new(&admission).is_err(),
        "legacy broker cannot gain a Fore route"
    );
    let mut ledger = CustodyLedger::<FACE_VALUE_BYTES>::new(&admission).unwrap();
    let payload = [7; FACE_VALUE_BYTES];
    assert_eq!(
        ledger.preflight_ingress(RemoteEndpointId(3), CordId(0), 0, &payload),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
    ledger
        .accepted_ingress(RemoteEndpointId(4), CordId(0), 0, &payload)
        .unwrap();
    assert_eq!(
        ledger.preflight_ingress(RemoteEndpointId(4), CordId(0), 0, &payload),
        Err(SchedulerError::RemoteSequenceRejected)
    );
    ledger
        .accepted_ingress(RemoteEndpointId(4), CordId(0), 1, &payload)
        .unwrap();
    assert_eq!(
        ledger.preflight_ingress(RemoteEndpointId(4), CordId(0), 2, &payload),
        Err(SchedulerError::OutputBlocked)
    );
    let value = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: FACE_VALUE_BYTES as u32,
    };
    let call = HostCallRequest {
        node: NodeId(0),
        request: RequestId(0),
        call: HostCallId(0),
        input: BoundedValueRef::new(value, FACE_VALUE_BYTES as u32).unwrap(),
    };
    let mut forged = payload;
    forged[0] = 8;
    assert_eq!(
        ledger.accept(call, &forged),
        Err(SchedulerError::ValueOwnershipViolation)
    );
    ledger.accept(call, &payload).unwrap();
    // Rejected pressure did not consume sequence2. Its exact next receipt fits.
    ledger
        .accepted_ingress(RemoteEndpointId(4), CordId(0), 2, &payload)
        .unwrap();
    ledger.accept(call, &payload).unwrap();
    ledger.accept(call, &payload).unwrap();
    assert_eq!(
        ledger.accept(call, &payload),
        Err(SchedulerError::ValueOwnershipViolation)
    );
    ledger.clear();
    assert_eq!(
        ledger.preflight_ingress(RemoteEndpointId(4), CordId(0), 3, &payload),
        Err(SchedulerError::InvalidRemoteCordAccess)
    );
}
