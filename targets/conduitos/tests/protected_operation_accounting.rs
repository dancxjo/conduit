//! Deterministic existing-envelope accounting; does not install IRQ capture.
use conduit_core::{
    OperationRefusal, ProtectedEntryAccounting, ProtectedEntryBudget, ProtectedOperation,
    SliceReturn,
};
use conduitos_compositor_core::wire::{
    decode_request, decode_response, encode_request, encode_response, Envelope, Request, Response,
};
#[test]
fn existing_wire_envelope_and_immutable_bytes_remain_the_only_operation_identity() {
    let policy = ProtectedEntryBudget {
        accounting: ProtectedEntryAccounting::SourceInterruptsV1,
        quantum_interrupts: 2,
        total_interrupts: 5,
    };
    let envelope = Envelope {
        instance: 7,
        request: 12,
    };
    let mut command_buffer = [0_u8; 64];
    let request = Request::Cursor {
        position: None,
        hover: false,
    };
    let command_len = encode_request(envelope, request, &mut command_buffer).unwrap();
    let command = &command_buffer[..command_len];
    assert_eq!(decode_request(command).unwrap(), (envelope, request));
    let mut operation = ProtectedOperation::new(policy).unwrap();
    operation.begin(envelope, command).unwrap();
    let slice = operation.grant(&envelope, command).unwrap();
    assert_eq!(
        operation.grant(
            &Envelope {
                instance: 8,
                ..envelope
            },
            command
        ),
        Err(OperationRefusal::WrongRequest)
    );
    operation
        .finish_slice(&envelope, command, slice, 2, SliceReturn::Preempted)
        .unwrap();
    assert_eq!(operation.remaining(), 3);
    let slice = operation.grant(&envelope, command).unwrap();
    operation
        .finish_slice(&envelope, command, slice, 1, SliceReturn::Gate)
        .unwrap();
    // The actual owner must validate complete response length/kind/envelope.
    // This fixture models that existing check; a Gate alone never calls it.
    let mut reply = [0_u8; 32];
    let length = encode_response(envelope, Response::Unit, &mut reply).unwrap();
    operation
        .complete(&envelope, command, |request, bytes| {
            decode_response(&reply[..length]).is_ok_and(|(observed, response)| {
                observed == *request && response == Response::Unit && bytes == command
            })
        })
        .unwrap();
    assert!(operation.is_terminal());
    assert_eq!(
        operation.begin(
            Envelope {
                request: 13,
                ..envelope
            },
            command
        ),
        Err(OperationRefusal::Terminal)
    );
}
