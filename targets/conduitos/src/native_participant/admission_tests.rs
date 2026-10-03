//! In-memory carrier fixture; real Noise authentication and canonical owner admission.
extern crate std;

use super::*;
use alloc::{collections::VecDeque, rc::Rc, vec, vec::Vec};
use conduit_body::{
    AdmissionManager, AdmissionSigns, Body, BodyMembership, INVITATION_SCHEMA,
    SPAWN_ADMISSION_RECEIPT_SCHEMA,
};
use conduit_core::{OfferGeneration, SignId};
use conduit_protected_line::{
    CarrierFailure, EndpointBinding, ProtectedHandshake, ProtectedSessionPolicy, SessionLimits,
};
use core::cell::RefCell;

const NOW: u64 = 1000;
const LIMITS: AdmissionByteLimits = AdmissionByteLimits {
    maximum_request_bytes: 4096,
    maximum_receipt_bytes: 4096,
};

struct Fixture {
    invitation: PortableInvitation,
    advertisement: HostAdvertisement,
    binding: SessionBinding,
    manager: AdmissionManager,
    membership: BodyMembership,
}

fn fixture() -> Fixture {
    let body = Body::born(
        "source/native".into(),
        "checked/native".into(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            NOW,
            NOW + 1000,
        )
        .unwrap();
    Fixture {
        invitation: PortableInvitation {
            schema: INVITATION_SCHEMA.into(),
            claim: invitation.claim(),
            secret: invitation.secret.copy_for_target_provisioning(),
            rendezvous: None,
        },
        advertisement: HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "host/native".into(),
            boot_id: "boot/native".into(),
            offer_generation: OfferGeneration(1),
            profile: "profile/native".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        },
        binding: SessionBinding {
            initiator: EndpointBinding {
                host_id: "host/native".into(),
                boot_id: "boot/native".into(),
            },
            responder: EndpointBinding {
                host_id: "host/owner".into(),
                boot_id: "boot/owner".into(),
            },
            negotiation_id: "negotiation/explicitly-authorized-fixture".into(),
            line_session_id: "session/one-invitation".into(),
            candidate_binding: "candidate/authorized-owner".into(),
            transport_binding: "carrier/in-memory-fixture".into(),
        },
        manager,
        membership: BodyMembership::new(body.body_id).unwrap(),
    }
}

fn prepare(f: &Fixture) -> PreparedNativeAdmission<'_> {
    PreparedNativeAdmission::prepare_authorized(
        &f.invitation,
        f.advertisement.clone(),
        &f.binding,
        Role::Initiator,
        LIMITS,
        NOW,
    )
    .unwrap()
}

type Queue = Rc<RefCell<VecDeque<Vec<u8>>>>;
struct FixtureCarrier {
    inbound: Queue,
    outbound: Queue,
}
impl ProtectedFrameCarrier for FixtureCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        self.outbound.borrow_mut().push_back(frame.to_vec());
        Ok(())
    }
    fn receive_frame(&mut self, output: &mut [u8], _: u32) -> Result<usize, CarrierFailure> {
        let frame = self
            .inbound
            .borrow_mut()
            .pop_front()
            .ok_or(CarrierFailure::Lost)?;
        if frame.len() > output.len() {
            return Err(CarrierFailure::Pressure);
        }
        output[..frame.len()].copy_from_slice(&frame);
        Ok(frame.len())
    }
    fn close(&mut self) -> Result<(), CarrierFailure> {
        Ok(())
    }
}

fn protected_pair(
    binding: &SessionBinding,
) -> (
    ProtectedCarrier<FixtureCarrier>,
    ProtectedCarrier<FixtureCarrier>,
) {
    protected_pair_tapped(binding, Queue::default())
}

fn protected_pair_tapped(
    binding: &SessionBinding,
    to_local: Queue,
) -> (
    ProtectedCarrier<FixtureCarrier>,
    ProtectedCarrier<FixtureCarrier>,
) {
    let policy = ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 4096,
            maximum_frames_per_direction: 2,
            maximum_bytes_per_direction: 8192,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 100,
        idle_timeout_millis: 100,
    };
    // The fixture supplies a separately authorized PSK. Invitation bytes are not session keys.
    let mut local =
        ProtectedHandshake::new(Role::Initiator, binding, policy.traffic, [31; 32], [32; 32])
            .unwrap();
    let mut owner =
        ProtectedHandshake::new(Role::Responder, binding, policy.traffic, [31; 32], [33; 32])
            .unwrap();
    let mut bytes = vec![0; local.next_message_bytes().unwrap()];
    local.write_message(&mut bytes).unwrap();
    owner.read_message(&bytes).unwrap();
    let mut bytes = vec![0; owner.next_message_bytes().unwrap()];
    owner.write_message(&mut bytes).unwrap();
    local.read_message(&bytes).unwrap();
    let to_owner = Queue::default();
    (
        ProtectedCarrier::new(
            FixtureCarrier {
                inbound: to_local.clone(),
                outbound: to_owner.clone(),
            },
            local.finish().unwrap(),
            policy,
        )
        .unwrap(),
        ProtectedCarrier::new(
            FixtureCarrier {
                inbound: to_owner,
                outbound: to_local,
            },
            owner.finish().unwrap(),
            policy,
        )
        .unwrap(),
    )
}

fn owner_receipt(
    manager: &mut AdmissionManager,
    membership: &mut BodyMembership,
    owner: &mut ProtectedCarrier<FixtureCarrier>,
) -> PortableAdmissionReceipt {
    let request: PortableSpawnAdmissionRequest =
        serde_json::from_slice(owner.receive().unwrap()).unwrap();
    let credential = manager
        .complete_spawn(
            membership,
            &request.host_advertisement,
            &request.admission_proof().unwrap(),
            NOW + 1,
            AdmissionSigns {
                part_admitted: "sign/part".into(),
                host_attached: "sign/host".into(),
                candidate_admitted: "sign/candidate".into(),
            },
        )
        .unwrap();
    PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential,
        host_advertisement: request.host_advertisement,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    }
}

#[test]
fn signed_request_is_only_preparation_until_real_owner_receipt() {
    let mut f = fixture();
    let binding = f.binding.clone();
    let prepared = PreparedNativeAdmission::prepare_authorized(
        &f.invitation,
        f.advertisement.clone(),
        &binding,
        Role::Initiator,
        LIMITS,
        NOW,
    )
    .unwrap();
    let request_bytes = prepared.request_bytes().to_vec();
    assert!(!prepared.request().membership_admitted);
    assert!(!prepared.request().plan_created && !prepared.request().play_created);
    assert!(request_bytes.len() > 64);
    let (mut local, mut owner) = protected_pair(&binding);
    let pending = prepared.send(&mut local, &f.advertisement, NOW).unwrap();
    let receipt = owner_receipt(&mut f.manager, &mut f.membership, &mut owner);
    let receipt_bytes = serde_json::to_vec(&receipt).unwrap();
    assert!(receipt_bytes.len() > 64);
    owner.send(&receipt_bytes).unwrap();
    let admitted = pending.complete(&f.advertisement, NOW + 1).unwrap();
    assert_eq!(admitted.credential, receipt.credential);
    assert_eq!(admitted.host_advertisement, f.advertisement);
    // A new pending request cannot reuse this completed session or its receipt.
    assert!(matches!(
        prepare(&f).send(&mut local, &f.advertisement, NOW + 2),
        Err(NativeAdmissionRefusal::OwnerSession)
    ));
    std::println!(
        "native admission fixture JSON request={} receipt={} bytes (protected and stream overhead excluded)",
        request_bytes.len(),
        receipt_bytes.len()
    );
}

#[test]
fn exact_encoded_request_limit_and_invalid_bounds_are_checked() {
    let f = fixture();
    let length = prepare(&f).request_bytes().len();
    for (limit, success) in [(length, true), (length - 1, false), (64, false)] {
        let result = PreparedNativeAdmission::prepare_authorized(
            &f.invitation,
            f.advertisement.clone(),
            &f.binding,
            Role::Initiator,
            AdmissionByteLimits {
                maximum_request_bytes: limit,
                ..LIMITS
            },
            NOW,
        );
        assert_eq!(result.is_ok(), success);
        if !success {
            assert!(matches!(
                result,
                Err(NativeAdmissionRefusal::RequestPressure)
            ));
        }
    }
    for limit in [0, 65_520] {
        assert!(matches!(
            PreparedNativeAdmission::prepare_authorized(
                &f.invitation,
                f.advertisement.clone(),
                &f.binding,
                Role::Initiator,
                AdmissionByteLimits {
                    maximum_receipt_bytes: limit,
                    ..LIMITS
                },
                NOW
            ),
            Err(NativeAdmissionRefusal::Bounds)
        ));
    }
}

#[test]
fn changed_host_boot_generation_and_offers_retire_before_send_or_receive() {
    let f = fixture();
    for mutation in 0..4 {
        let mut changed = f.advertisement.clone();
        match mutation {
            0 => changed.host_id = "host/replaced".into(),
            1 => changed.boot_id = "boot/replaced".into(),
            2 => changed.offer_generation.0 += 1,
            _ => changed.profile = "profile/replaced".into(),
        }
        let (mut local, _) = protected_pair(&f.binding);
        assert!(matches!(
            prepare(&f).send(&mut local, &changed, NOW),
            Err(NativeAdmissionRefusal::StaleHost)
        ));
        assert_eq!(local.session().evidence().sent_frames, 0);
        let pending = prepare(&f).send(&mut local, &f.advertisement, NOW).unwrap();
        assert!(matches!(
            pending.complete(&changed, NOW + 1),
            Err(NativeAdmissionRefusal::StaleHost)
        ));
        assert_eq!(local.session().evidence().received_frames, 0);
    }
}

#[test]
fn wrong_authenticated_owner_binding_and_expiry_are_refused() {
    let f = fixture();
    let mut wrong = f.binding.clone();
    wrong.responder.boot_id = "boot/substituted".into();
    let (mut local, _) = protected_pair(&wrong);
    assert!(matches!(
        prepare(&f).send(&mut local, &f.advertisement, NOW),
        Err(NativeAdmissionRefusal::OwnerSession)
    ));
    let (mut local, _) = protected_pair(&f.binding);
    for now in [NOW - 1, NOW + 1000] {
        assert!(matches!(
            prepare(&f).send(&mut local, &f.advertisement, now),
            Err(NativeAdmissionRefusal::Expired)
        ));
    }
    let pending = prepare(&f).send(&mut local, &f.advertisement, NOW).unwrap();
    assert!(matches!(
        pending.complete(&f.advertisement, NOW + 1000),
        Err(NativeAdmissionRefusal::Expired)
    ));
}

#[test]
fn malformed_substituted_or_effect_claiming_receipt_is_terminal() {
    for mutation in 0..7 {
        let mut f = fixture();
        let binding = f.binding.clone();
        let prepared = PreparedNativeAdmission::prepare_authorized(
            &f.invitation,
            f.advertisement.clone(),
            &binding,
            Role::Initiator,
            LIMITS,
            NOW,
        )
        .unwrap();
        let (mut local, mut owner) = protected_pair(&binding);
        let pending = prepared.send(&mut local, &f.advertisement, NOW).unwrap();
        let mut receipt = owner_receipt(&mut f.manager, &mut f.membership, &mut owner);
        match mutation {
            0 => receipt.credential.boot_id = "boot/substituted".into(),
            1 => receipt.host_advertisement.offer_generation.0 += 1,
            2 => receipt.plan_created = true,
            3 => receipt.play_created = true,
            4 => receipt.credential.issued_at_millis = NOW - 1,
            5 => receipt.credential.issued_at_millis = NOW + 2,
            _ => {}
        }
        let bytes = if mutation == 6 {
            b"not json".to_vec()
        } else {
            serde_json::to_vec(&receipt).unwrap()
        };
        owner.send(&bytes).unwrap();
        assert!(pending.complete(&f.advertisement, NOW + 1).is_err());
        assert!(matches!(
            prepare(&f).send(&mut local, &f.advertisement, NOW + 2),
            Err(NativeAdmissionRefusal::OwnerSession)
        ));
    }
}

#[test]
fn receipt_pressure_cancel_and_carrier_loss_do_not_admit() {
    let f = fixture();
    let prepared = PreparedNativeAdmission::prepare_authorized(
        &f.invitation,
        f.advertisement.clone(),
        &f.binding,
        Role::Initiator,
        AdmissionByteLimits {
            maximum_receipt_bytes: 64,
            ..LIMITS
        },
        NOW,
    )
    .unwrap();
    let (mut local, mut owner) = protected_pair(&f.binding);
    let pending = prepared.send(&mut local, &f.advertisement, NOW).unwrap();
    owner.send(&[b' '; 65]).unwrap();
    assert!(matches!(
        pending.complete(&f.advertisement, NOW + 1),
        Err(NativeAdmissionRefusal::ReceiptPressure)
    ));
    let (mut local, _) = protected_pair(&f.binding);
    let pending = prepare(&f).send(&mut local, &f.advertisement, NOW).unwrap();
    pending.cancel().unwrap();
    assert_eq!(
        local.session().evidence().disposition,
        SessionDisposition::Closed
    );
    let (mut local, _) = protected_pair(&f.binding);
    let pending = prepare(&f).send(&mut local, &f.advertisement, NOW).unwrap();
    assert!(matches!(
        pending.complete(&f.advertisement, NOW + 1),
        Err(NativeAdmissionRefusal::Protected(
            ProtectedLineError::OuterCarrierLost
        ))
    ));
    prepare(&f).cancel();
}

#[test]
fn forged_protected_frame_never_becomes_a_receipt() {
    let f = fixture();
    let inbound = Queue::default();
    let (mut local, mut owner) = protected_pair_tapped(&f.binding, inbound.clone());
    let pending = prepare(&f).send(&mut local, &f.advertisement, NOW).unwrap();
    owner.send(b"{}").unwrap();
    let mut frames = inbound.borrow_mut();
    let frame = frames.front_mut().unwrap();
    *frame.last_mut().unwrap() ^= 1;
    drop(frames);
    assert!(matches!(
        pending.complete(&f.advertisement, NOW + 1),
        Err(NativeAdmissionRefusal::Protected(
            ProtectedLineError::AuthenticationFailed
        ))
    ));
}

#[test]
fn actual_encoded_receipt_defines_the_boundary() {
    let mut exact_length = 0;
    for attempt in 0..3 {
        let mut f = fixture();
        let binding = f.binding.clone();
        let maximum = match attempt {
            0 => 4096,
            1 => exact_length,
            _ => exact_length - 1,
        };
        let prepared = PreparedNativeAdmission::prepare_authorized(
            &f.invitation,
            f.advertisement.clone(),
            &binding,
            Role::Initiator,
            AdmissionByteLimits {
                maximum_receipt_bytes: maximum,
                ..LIMITS
            },
            NOW,
        )
        .unwrap();
        let (mut local, mut owner) = protected_pair(&binding);
        let pending = prepared.send(&mut local, &f.advertisement, NOW).unwrap();
        let receipt = owner_receipt(&mut f.manager, &mut f.membership, &mut owner);
        let bytes = serde_json::to_vec(&receipt).unwrap();
        exact_length = bytes.len();
        owner.send(&bytes).unwrap();
        let result = pending.complete(&f.advertisement, NOW + 1);
        if attempt == 2 {
            assert!(matches!(
                result,
                Err(NativeAdmissionRefusal::ReceiptPressure)
            ));
        } else {
            assert!(result.is_ok());
        }
    }
}
