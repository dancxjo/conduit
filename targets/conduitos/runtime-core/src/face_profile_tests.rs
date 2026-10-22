//! Explicit profile and wire-boundary conformance; no native platform claim.
extern crate std;

fn on_host_stack(body: impl FnOnce() + Send + 'static) {
    // Debug Runtime/Owner construction copies exceed the harness default stack.
    // This hosted-only allowance says nothing about the native 1 MiB stack.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(body)
        .unwrap()
        .join()
        .unwrap();
}
use crate::{
    admission::{Admission, BackKind},
    runtime::Runtime,
    wire::{self, Envelope, Request, Response},
    *,
};
use conduit_kernel::{
    scheduler::{CordCapacity, CordSpec, RemoteIngressOutcome},
    *,
};

fn admission(maximum: u32) -> Admission {
    let mut admission = Admission::new(1, 1);
    admission.backs[0] = BackKind::Face;
    admission.nodes[0].maximum_step_fuel = 16;
    admission.nodes[0].input_cords[0] = Some(CordId(0));
    admission.cords[0] = CordSpec::remote_ingress(
        CordId(0),
        RemoteEndpointId(0),
        (NodeId(0), PortId(0)),
        CordCapacity {
            slot_start: 0,
            item_capacity: 1,
            byte_capacity: maximum,
            pressure_policy: Default::default(),
        },
    )
    .with_maximum_value_bytes(maximum);
    admission.bindings[0] = Some(HostCallBinding {
        call: HostCallId(0),
        maximum_input_bytes: maximum,
        maximum_output_bytes: 0,
    });
    admission
}
fn initialize(runtime: &mut Runtime, face: bool, maximum: u32) {
    let mut transfer = [0; 16 * 1024];
    let bytes = wire::admission::encode(&admission(maximum), &mut transfer).unwrap() as u32;
    let request = if face {
        Request::InitFace { bytes }
    } else {
        Request::Init { bytes }
    };
    assert_eq!(
        runtime.execute(request, &mut transfer).unwrap(),
        Response::Ready
    );
}
fn ingress(sequence: u64, bytes: u32) -> Request {
    Request::AdmitRemote {
        endpoint: RemoteEndpointId(0),
        cord: CordId(0),
        sequence,
        bytes,
    }
}
#[test]
fn face_profile_admits_exact_8192_fore_value_and_keeps_pressure_sequence_typed() {
    on_host_stack(|| {
        let mut runtime = Runtime::default();
        initialize(&mut runtime, true, 8192);
        let mut bytes = [0x65; 8193];
        assert!(runtime.execute(ingress(0, 8193), &mut bytes).is_err());
        assert!(
            runtime
                .execute(
                    Request::AdmitRemote {
                        endpoint: RemoteEndpointId(1),
                        cord: CordId(0),
                        sequence: 0,
                        bytes: 8192
                    },
                    &mut bytes
                )
                .is_err()
        );
        assert_eq!(
            runtime.execute(ingress(0, 8192), &mut bytes).unwrap(),
            Response::RemoteAdmission(RemoteIngressOutcome::Accepted { sequence: 0 })
        );
        assert!(runtime.execute(ingress(0, 8192), &mut bytes).is_err());
        assert_eq!(
            runtime.execute(ingress(1, 8192), &mut bytes).unwrap(),
            Response::RemoteAdmission(RemoteIngressOutcome::Full { sequence: 1 })
        );
        let request = (0..16)
            .find_map(|_| {
                runtime.execute(Request::Step, &mut bytes).unwrap();
                match runtime.execute(Request::Next, &mut bytes).unwrap() {
                    Response::Next(Some(request)) => Some(request),
                    _ => None,
                }
            })
            .expect("bounded Face sink yields exact Host Call");
        assert_eq!(request.input.value.byte_len, 8192);
        assert_eq!(&bytes[..8192], &[0x65; 8192]);
        // Full did not advance sequence1; after the queued value reaches its Host
        // Call, this exact next sequence remains admissible into the finite queue.
        assert_eq!(
            runtime.execute(ingress(1, 8192), &mut bytes).unwrap(),
            Response::RemoteAdmission(RemoteIngressOutcome::Accepted { sequence: 1 })
        );
        runtime
            .execute(
                Request::Complete {
                    node: request.node,
                    request: request.request,
                    outcome: HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                },
                &mut bytes,
            )
            .unwrap();
        // Sequence1 is already queued. Successfully dispatch more than ten inputs
        // on this one endpoint: retained remote Sign events must not be capped at
        // the number of possible Cords/endpoints. Storage remains fixed throughout.
        for sequence in 1..=12 {
            if sequence > 1 {
                assert_eq!(
                    runtime
                        .execute(ingress(sequence, 8192), &mut bytes)
                        .unwrap(),
                    Response::RemoteAdmission(RemoteIngressOutcome::Accepted { sequence })
                );
            }
            let pending = (0..16)
                .find_map(|_| {
                    runtime.execute(Request::Step, &mut bytes).unwrap();
                    match runtime.execute(Request::Next, &mut bytes).unwrap() {
                        Response::Next(Some(request)) => Some(request),
                        _ => None,
                    }
                })
                .expect("each admitted input reaches the exact pending Face Host Call");
            assert_eq!(pending.request, RequestId(sequence as u32));
            assert_eq!(pending.input.value.byte_len, 8192);
            runtime
                .execute(
                    Request::Complete {
                        node: pending.node,
                        request: pending.request,
                        outcome: HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    },
                    &mut bytes,
                )
                .unwrap();
        }
        runtime.execute(Request::Cancel, &mut bytes).unwrap();
    });
}
#[test]
fn legacy_profile_keeps_3072_and_refuses_new_ingress_before_mutation() {
    on_host_stack(|| {
        assert_eq!(VALUE_BYTES, 3072);
        let mut runtime = Runtime::default();
        initialize(&mut runtime, false, 3072);
        let mut bytes = [0; 8192];
        assert!(runtime.execute(ingress(0, 8), &mut bytes).is_err());
        assert!(
            runtime
                .execute(Request::Store { bytes: 3073 }, &mut bytes)
                .is_err()
        );
        assert!(matches!(
            runtime.execute(Request::Store { bytes: 3072 }, &mut bytes),
            Ok(Response::Stored(_))
        ));
        runtime.execute(Request::Cancel, &mut bytes).unwrap();
    });
}
#[test]
fn face_admission_refuses_oversized_contracts_and_remote_egress() {
    assert!(admission(8193).validate_face().is_err());
    let mut invalid = admission(8192);
    invalid.bindings[0].as_mut().unwrap().maximum_output_bytes = 8193;
    assert!(invalid.validate_face().is_err());
    let mut invalid = admission(8192);
    invalid.cords[0].sink = CordEndpoint::Remote(RemoteEndpointId(1));
    assert!(invalid.validate_face().is_err());
}
#[test]
fn additive_face_wire_requests_and_pressure_responses_roundtrip_strictly() {
    let envelope = Envelope {
        instance: 1,
        request: 1,
    };
    let mut bytes = [0; wire::EXCHANGE_BYTES];
    for request in [Request::InitFace { bytes: 100 }, ingress(u64::MAX, 8192)] {
        let len = wire::encode_request(envelope, request, &mut bytes).unwrap();
        assert_eq!(
            wire::decode_request(&bytes[..len]).unwrap(),
            (envelope, request)
        );
        assert!(wire::decode_request(&bytes[..len - 1]).is_err());
    }
    for outcome in [
        RemoteIngressOutcome::Accepted { sequence: 7 },
        RemoteIngressOutcome::Full { sequence: 7 },
    ] {
        let response = Response::RemoteAdmission(outcome);
        let len = wire::encode_response(envelope, response, None, &mut bytes).unwrap();
        assert_eq!(
            wire::decode_response(&bytes[..len]).unwrap(),
            (envelope, response, None)
        );
        assert!(response.matches(ingress(7, 8192)));
        assert!(!response.matches(Request::Step));
        assert!(!Response::Ack.matches(ingress(7, 8192)));
        bytes[33] = 2; // None gap flag, followed by the strict remote outcome tag.
        assert!(wire::decode_response(&bytes[..len]).is_err());
    }
}
