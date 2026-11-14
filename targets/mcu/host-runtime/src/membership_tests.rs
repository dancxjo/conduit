use super::membership::{Refusal, SpawnAdmission};
use super::tests::ready;
use super::transport::MessageTransport;
use conduit_body::*;
use conduit_core::{BootId, HostAdvertisement, HostId, HostProfileId, OfferGeneration};
use ed25519_dalek::{Signer, SigningKey};
use std::{collections::VecDeque, vec, vec::Vec};

struct Messages {
    incoming: VecDeque<Vec<u8>>,
    sent: usize,
}
impl MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.incoming.pop_front().ok_or(())?;
        let target = output.get_mut(..bytes.len()).ok_or(())?;
        target.copy_from_slice(&bytes);
        Ok(target)
    }
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        self.sent += 1;
        Ok(())
    }
}
pub(crate) fn fixture() -> (
    MemberBootstrap<'static>,
    HostAdvertisement,
    SigningKey,
    SigningKey,
) {
    let owner = SigningKey::from_bytes(&[7; 32]);
    let member = SigningKey::from_bytes(&[3; 32]);
    let bootstrap = MemberBootstrap {
        schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: serde_json::from_str(r#""body/1""#).unwrap(),
        host_id: "host/1".into(),
        image_id: "image/1".into(),
        configuration_id: "configuration/1".into(),
        owner: FederationPeer {
            host_id: HostId::from("owner/1"),
            boot_id: BootId::from("owner/boot"),
            offer_generation: OfferGeneration(1),
            verifying_key: owner.verifying_key().to_bytes(),
        },
        admission: MemberBootstrapMaterial::Spawn {
            provision: PicoSpawnProvision {
                protocol: PICO_SPAWN_PROTOCOL,
                spore_id: "spore/1",
                image_id: "image/1",
                invitation_id: "invitation/1",
                body_id: "body/1",
                nonce: [2; 32],
                expires_at_millis: 1000,
                secret: [3; 32],
            },
        },
    };
    let actual = HostAdvertisement {
        declared_name: None,
        protocol_version: 1,
        host_id: HostId::from("host/1"),
        boot_id: BootId::from("esp32/actual-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("configuration/1"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    (bootstrap, actual, member, owner)
}
pub(crate) fn receipt(
    bootstrap: &MemberBootstrap<'_>,
    actual: &HostAdvertisement,
) -> PortableAdmissionReceipt {
    PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential: MembershipCredential {
            credential_id: serde_json::from_str(r#""credential/1""#).unwrap(),
            body_id: bootstrap.body_id.clone(),
            part_id: serde_json::from_str(r#""part/1""#).unwrap(),
            host_id: actual.host_id.clone(),
            boot_id: actual.boot_id.clone(),
            issued_at_millis: 10,
        },
        host_advertisement: actual.clone(),
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    }
}
fn messages(
    receipt: &PortableAdmissionReceipt,
    bootstrap: &MemberBootstrap<'_>,
    member: &SigningKey,
    owner: &SigningKey,
) -> Messages {
    let signed = attest_portable_admission_receipt(
        receipt,
        bootstrap.owner.clone(),
        member.verifying_key().to_bytes(),
        |t| owner.sign(t).to_bytes(),
    )
    .unwrap();
    Messages {
        incoming: [
            serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
            signed.receipt_json.into_bytes(),
        ]
        .into(),
        sent: 0,
    }
}
#[test]
fn shared_exchange_accepts_only_independently_signed_current_boot_membership() {
    let (bootstrap, actual, member, owner) = fixture();
    let receipt = receipt(&bootstrap, &actual);
    let mut link = messages(&receipt, &bootstrap, &member, &owner);
    let exchange = SpawnAdmission {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        current: None,
        freshness: 1,
        friendly_label: "ESP32 Host",
    };
    let mut input = [0; 4096];
    let mut output = [0; 4096];
    let accepted = ready(exchange.exchange(&mut link, &mut input, &mut output)).unwrap();
    assert_eq!(accepted.credential.part_id.as_str(), "part/1");
    assert_eq!(accepted.credential.boot_id, actual.boot_id);
    assert_eq!(link.sent, 2);
    assert!(input.iter().all(|b| *b == 0) && output.iter().all(|b| *b == 0));
}
#[test]
fn stale_signed_receipt_and_transport_interruption_cannot_install_membership() {
    let (bootstrap, actual, member, owner) = fixture();
    let mut stale = actual.clone();
    stale.boot_id = BootId::from("old/boot");
    let mut link = messages(&receipt(&bootstrap, &stale), &bootstrap, &member, &owner);
    let exchange = SpawnAdmission {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        current: None,
        freshness: 1,
        friendly_label: "ESP32 Host",
    };
    assert!(matches!(
        ready(exchange.exchange(&mut link, &mut [0; 4096], &mut [0; 4096])),
        Err(Refusal::Receipt)
    ));
    let mut interrupted = Messages {
        incoming: VecDeque::new(),
        sent: 0,
    };
    assert!(matches!(
        ready(exchange.exchange(&mut interrupted, &mut [0; 4096], &mut [0; 4096])),
        Err(Refusal::Transport)
    ));
}
#[test]
fn current_member_and_wrong_native_key_refuse_before_carrier_effects() {
    let (bootstrap, actual, member, _) = fixture();
    let credential = receipt(&bootstrap, &actual).credential;
    let mut link = Messages {
        incoming: VecDeque::new(),
        sent: 0,
    };
    let mut exchange = SpawnAdmission {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/1",
        key: &member,
        current: Some(&credential),
        freshness: 1,
        friendly_label: "ESP32 Host",
    };
    assert!(matches!(
        ready(exchange.exchange(&mut link, &mut [0; 4096], &mut [0; 4096])),
        Err(Refusal::RetainedMember)
    ));
    let wrong = SigningKey::from_bytes(&[9; 32]);
    exchange.current = None;
    exchange.key = &wrong;
    assert!(matches!(
        ready(exchange.exchange(&mut link, &mut [0; 4096], &mut [0; 4096])),
        Err(Refusal::Scope)
    ));
    assert_eq!(link.sent, 0);
}

#[test]
fn mismatched_running_image_and_provision_body_or_image_refuse_before_io() {
    let (bootstrap, actual, member, _) = fixture();
    let mut link = Messages {
        incoming: VecDeque::new(),
        sent: 0,
    };
    let exchange = SpawnAdmission {
        bootstrap: &bootstrap,
        actual: &actual,
        image_id: "image/other",
        key: &member,
        current: None,
        freshness: 1,
        friendly_label: "ESP32 Host",
    };
    assert!(matches!(
        ready(exchange.exchange(&mut link, &mut [0; 4096], &mut [0; 4096])),
        Err(Refusal::Scope)
    ));
    for body_mismatch in [true, false] {
        let (mut bootstrap, actual, member, _) = fixture();
        if let MemberBootstrapMaterial::Spawn { provision } = &mut bootstrap.admission {
            if body_mismatch {
                provision.body_id = "body/other";
            } else {
                provision.image_id = "image/other";
            }
        }
        let exchange = SpawnAdmission {
            bootstrap: &bootstrap,
            actual: &actual,
            image_id: "image/1",
            key: &member,
            current: None,
            freshness: 1,
            friendly_label: "ESP32 Host",
        };
        assert!(matches!(
            ready(exchange.exchange(&mut link, &mut [0; 4096], &mut [0; 4096])),
            Err(Refusal::Scope)
        ));
    }
    assert_eq!(link.sent, 0);
}
