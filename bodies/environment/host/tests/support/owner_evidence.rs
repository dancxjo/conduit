//! Exact owner acknowledgement authentication; no transport or MCU is claimed.
use crate::native_owner_control::*;
use ed25519_dalek::{Signer, SigningKey};
fn request(sequence: u64) -> Request<'static> {
    Request {
        protocol: PROTOCOL,
        operation: Operation::AcceptEvidence,
        sequence,
        host: "host",
        boot: "boot",
        offer_generation: 3,
        body: "body",
        wake: "wake",
        body_plan: "body-plan",
        body_play: "body-play",
        plan: "plan",
        fragment: "fragment",
        fragment_play: "fragment-play",
        assignment_seal: [1; 32],
        evidence: Some(EvidenceReference {
            invocation: 7,
            sha256: [2; 32],
        }),
    }
}
#[test]
fn evidence_ack_binds_scope_page_digest_and_shared_owner_replay_sequence() {
    let key = SigningKey::from_bytes(&[31; 32]);
    let mut control = OwnerControl::new(Some(key.verifying_key().to_bytes())).unwrap();
    let signature = key.sign(&request(1).transcript().unwrap()).to_bytes();
    for field in 0..5 {
        let mut changed = request(1);
        match field {
            0 => changed.evidence.as_mut().unwrap().invocation += 1,
            1 => changed.evidence.as_mut().unwrap().sha256[0] ^= 1,
            2 => changed.body = "foreign",
            3 => changed.fragment_play = "foreign",
            _ => changed.assignment_seal[0] ^= 1,
        }
        assert!(matches!(
            control.authenticate(changed, &signature, "host", "boot", 3),
            Err(Refusal::Signature)
        ));
        assert_eq!(control.last_sequence(), 0);
    }
    control
        .authenticate(request(1), &signature, "host", "boot", 3)
        .unwrap();
    assert!(matches!(
        control.authenticate(request(1), &signature, "host", "boot", 3),
        Err(Refusal::Replay)
    ));
    let mut inspect = request(2);
    inspect.operation = Operation::Inspect;
    inspect.evidence = None;
    let inspect_signature = key.sign(&inspect.transcript().unwrap()).to_bytes();
    control
        .authenticate(inspect, &inspect_signature, "host", "boot", 3)
        .unwrap();
    assert!(matches!(
        control.authenticate(request(1), &signature, "host", "boot", 3),
        Err(Refusal::Replay)
    ));
}
#[test]
fn acknowledgement_payload_cannot_be_reinterpreted_as_an_ordinary_command() {
    let mut ack = request(1);
    ack.evidence = None;
    assert_eq!(ack.transcript(), Err(Refusal::Malformed));
    for operation in [
        Operation::Inspect,
        Operation::Reserve,
        Operation::Prepare,
        Operation::Start,
        Operation::Release,
        Operation::Cancel,
        Operation::Retire,
    ] {
        let mut wrong = request(1);
        wrong.operation = operation;
        assert_eq!(wrong.transcript(), Err(Refusal::Malformed));
    }
    let mut empty = request(1);
    empty.evidence.as_mut().unwrap().sha256 = [0; 32];
    assert_eq!(empty.transcript(), Err(Refusal::Malformed));
}

#[path = "../../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[test]
fn page_custody_survives_pressure_wrong_acceptance_and_terminal_extension_without_allocation() {
    use crate::native_display_evidence::*;
    use conduit_kernel::{self as k, SignSink};
    let mut log = fresh_log().unwrap();
    log.record(
        k::NodeId(3),
        Some(k::PortId(0)),
        Some(k::RequestId(17)),
        k::KernelEventKind::HostCallRequested,
    )
    .unwrap();
    log.record(
        k::NodeId(3),
        None,
        Some(k::RequestId(17)),
        k::KernelEventKind::HostCallCompleted,
    )
    .unwrap();
    log.record_remote(
        k::NodeId(2),
        k::PortId(0),
        k::KernelEventKind::RemoteOutputClosed,
        k::RemoteLifecycleIdentity {
            endpoint: k::RemoteEndpointId(1),
            cord: k::CordId(4),
            direction: k::RemoteCordDirection::Egress,
            sequence: 9,
        },
    )
    .unwrap();
    let mut custody = Custody::prepare();
    let key = SigningKey::from_bytes(&[19; 32]);
    let mut bytes = [0; conduit_environment_runtime::display_evidence::MAXIMUM_BYTES];
    let ((), memory) = allocation_probe::observe(|| {
        assert!(custody.publish(5, &log).unwrap());
        let (page, length) = custody.copy(&mut bytes).unwrap().unwrap();
        assert!(custody.release_accepted().is_err());
        let mut wrong = page;
        wrong.invocation += 1;
        assert!(custody.accept(wrong).is_err());
        wrong = page;
        wrong.sha256[0] ^= 1;
        assert!(custody.accept(wrong).is_err());
        assert!(custody.publish(6, &log).is_err());
        assert_eq!(custody.pending(), Some(page));
        let offer = Offer {
            command: [4; 32],
            page,
            bytes: length as u16,
        };
        let signature = key.sign(&offer.transcript()).to_bytes();
        offer
            .verify(
                &key.verifying_key().to_bytes(),
                &[4; 32],
                &signature,
                &bytes[..length],
            )
            .unwrap();
        assert!(offer
            .verify(
                &key.verifying_key().to_bytes(),
                &[5; 32],
                &signature,
                &bytes[..length]
            )
            .is_err());
        bytes[16] ^= 1;
        assert!(offer
            .verify(
                &key.verifying_key().to_bytes(),
                &[4; 32],
                &signature,
                &bytes[..length]
            )
            .is_err());
        custody.accept(page).unwrap();
        assert!(custody.accept(page).is_err());
        assert_eq!(custody.release_accepted().unwrap(), page);
        assert!(!custody.publish(5, &log).unwrap());
        log.record(k::NodeId(0), None, None, k::KernelEventKind::RunCancelled)
            .unwrap();
        assert!(custody.publish(5, &log).unwrap());
        let extended = custody.pending().unwrap();
        assert_ne!(extended, page);
        assert!(custody.accept(page).is_err());
        custody.accept(extended).unwrap();
        custody.release_accepted().unwrap();
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn evidence_form_preserves_every_event_kind_and_rejects_noncanonical_or_gapped_pages() {
    use crate::native_display_evidence::fresh_log;
    use conduit_environment_runtime::display_evidence as wire;
    use conduit_kernel::{self as k, KernelEventKind as K, SignQuery, SignSink};
    let mut log = fresh_log().unwrap();
    for kind in [
        K::StepFuelGranted,
        K::StepYielded,
        K::StepFuelExceeded,
        K::ValueStored,
        K::ValueRouted,
        K::ValueConsumed,
        K::RemoteValueOffered,
        K::RemoteValueAccepted,
        K::RemoteValueDelivered,
        K::RemoteOutputClosed,
        K::RemoteInputAdmitted,
        K::RemoteInputClosed,
        K::InputClosed,
        K::InputAbnormal,
        K::HostCallRequested,
        K::HostCallCancellationRequested,
        K::HostCallCompleted,
        K::BackCompleted,
        K::BackFailed,
        K::SemanticAbnormal,
        K::SemanticAbnormalRecovered,
        K::CancellationRequested,
        K::RunCancelled,
    ] {
        let remote = matches!(
            kind,
            K::RemoteValueOffered
                | K::RemoteValueAccepted
                | K::RemoteValueDelivered
                | K::RemoteOutputClosed
                | K::RemoteInputAdmitted
                | K::RemoteInputClosed
        );
        if remote {
            log.record_remote(
                k::NodeId(2),
                k::PortId(7),
                kind,
                k::RemoteLifecycleIdentity {
                    endpoint: k::RemoteEndpointId(3),
                    cord: k::CordId(4),
                    direction: if matches!(kind, K::RemoteInputAdmitted | K::RemoteInputClosed) {
                        k::RemoteCordDirection::Ingress
                    } else {
                        k::RemoteCordDirection::Egress
                    },
                    sequence: 1234,
                },
            )
            .unwrap();
        } else {
            log.record(k::NodeId(5), None, Some(k::RequestId(123)), kind)
                .unwrap();
        }
    }
    let mut bytes = [0; wire::MAXIMUM_BYTES];
    let size = wire::encode(8, log.events(), &log, &mut bytes).unwrap();
    let page = wire::Page::parse(&bytes[..size]).unwrap();
    assert_eq!(page.invocation(), 8);
    assert!(page.records().map(|r| r.event).eq(log.events()));
    for record in page.records() {
        assert_eq!(record.remote, log.remote_identity(record.event.sequence));
    }
    for at in [0, 4, 6, 16, 22, 23, 25, 30, 31, 45] {
        let mut bad = bytes;
        bad[at] = 255;
        assert!(wire::Page::parse(&bad[..size]).is_err(), "offset {at}");
    }
    assert!(wire::Page::parse(&bytes[..size - 1]).is_err());
    let mut gap = fresh_log().unwrap();
    for _ in 0..100 {
        gap.record(k::NodeId(0), None, None, K::StepFuelGranted)
            .unwrap();
    }
    assert!(gap.retention_gap().is_some());
    assert_eq!(
        wire::encode(9, gap.events(), &gap, &mut bytes),
        Err(wire::Refusal::Gap)
    );
    let empty = fresh_log().unwrap();
    let events = (0..65).map(|sequence| k::KernelEvent {
        sequence,
        node: k::NodeId(0),
        port: None,
        request: None,
        kind: K::ValueStored,
    });
    assert_eq!(
        wire::encode(9, events, &empty, &mut bytes),
        Err(wire::Refusal::Capacity)
    );
}
