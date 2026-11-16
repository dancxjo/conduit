//! Fresh CPU-only authority through the public Root admission exchange.
//! No live identity, receipt injection, resource admission or Started evidence.
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    root::{Configuration, Root},
    seed::Seed,
    transport::MessageTransport,
};
use ed25519_dalek::{Signer, SigningKey};
use std::{
    collections::VecDeque,
    future::Future,
    task::{Context, Poll, Waker},
};

pub const HOST: &str = "cpu-factory/host";
pub const BOOT: &str = "cpu-factory/boot/1";
pub struct Authority {
    pub bootstrap: b::MemberBootstrap<'static>,
    pub owner: SigningKey,
    pub member: SigningKey,
}
impl Authority {
    pub fn new() -> Self {
        let owner = SigningKey::from_bytes(&[91; 32]);
        let member = SigningKey::from_bytes(&[92; 32]);
        Self {
            owner: owner.clone(),
            member,
            bootstrap: b::MemberBootstrap {
                schema: b::MEMBER_BOOTSTRAP_SCHEMA.into(),
                body_id: serde_json::from_str("\"cpu-factory/body\"").unwrap(),
                host_id: HOST.into(),
                image_id: crate::config::IMAGE_ID.into(),
                configuration_id: crate::config::CONFIGURATION_ID.into(),
                owner: b::FederationPeer {
                    host_id: "cpu-factory/owner".into(),
                    boot_id: "cpu-factory/owner/boot".into(),
                    offer_generation: c::OfferGeneration(1),
                    verifying_key: owner.verifying_key().to_bytes(),
                },
                admission: b::MemberBootstrapMaterial::Spawn {
                    provision: b::PicoSpawnProvision {
                        protocol: b::PICO_SPAWN_PROTOCOL,
                        spore_id: "cpu-factory/spore",
                        image_id: crate::config::IMAGE_ID,
                        invitation_id: "cpu-factory/invitation",
                        body_id: "cpu-factory/body",
                        nonce: [93; 32],
                        expires_at_millis: 1000,
                        secret: [92; 32],
                    },
                },
            },
        }
    }
    pub fn seed(&self) -> Vec<u8> {
        let payload = serde_json::to_vec(&self.bootstrap).unwrap();
        let mut seed = b"CONDUIT_SEED@2\0\0".to_vec();
        seed.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        seed.extend(payload);
        seed
    }
    pub fn host(&self) -> c::HostAdvertisement {
        c::HostAdvertisement {
            declared_name: None,
            protocol_version: c::PROTOCOL_VERSION,
            host_id: HOST.into(),
            boot_id: BOOT.into(),
            offer_generation: c::OfferGeneration(1),
            profile: crate::config::CONFIGURATION_ID.into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        }
    }
    pub fn admit<const N: usize>(&self, root: &mut Root<'_, N>) {
        let receipt = b::PortableAdmissionReceipt {
            schema: b::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            credential: b::MembershipCredential {
                credential_id: serde_json::from_str("\"cpu-factory/credential\"").unwrap(),
                body_id: self.bootstrap.body_id.clone(),
                part_id: serde_json::from_str("\"cpu-factory/part\"").unwrap(),
                host_id: root.actual().host_id.clone(),
                boot_id: root.actual().boot_id.clone(),
                issued_at_millis: 10,
            },
            host_advertisement: root.actual().clone(),
            membership_admitted: true,
            current_offers_available: false,
            plan_created: false,
            play_created: false,
        };
        let signed = b::attest_portable_admission_receipt(
            &receipt,
            self.bootstrap.owner.clone(),
            self.member.verifying_key().to_bytes(),
            |bytes| self.owner.sign(bytes).to_bytes(),
        )
        .unwrap();
        let mut messages = Messages {
            incoming: [
                serde_json::to_vec(&signed.transport_metadata().unwrap()).unwrap(),
                signed.receipt_json.into_bytes(),
            ]
            .into(),
            sends: 0,
        };
        ready(root.spawn(&mut messages, &mut [0; 4096])).unwrap();
        assert!(root.has_current_member());
        assert!(!root.retained_input());
        assert_eq!(messages.sends, 2);
    }
}
struct Messages {
    incoming: VecDeque<Vec<u8>>,
    sends: usize,
}
impl MessageTransport for Messages {
    type Error = ();
    async fn receive<'a>(&mut self, out: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let msg = self.incoming.pop_front().ok_or(())?;
        let target = out.get_mut(..msg.len()).ok_or(())?;
        target.copy_from_slice(&msg);
        Ok(target)
    }
    async fn send(&mut self, _: &[u8]) -> Result<(), ()> {
        self.sends += 1;
        Ok(())
    }
}
pub fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    match future.as_mut().poll(&mut cx) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("finite CPU fixture unexpectedly pending"),
    }
}
pub fn verify_public_admission() {
    let authority = Authority::new();
    let seed = authority.seed();
    let mut root: Root<'_, { crate::config::MAX_ASSIGNMENT_BYTES }> = Root::new(
        Seed::parse(
            &seed,
            HOST,
            crate::config::IMAGE_ID,
            crate::config::CONFIGURATION_ID,
        )
        .unwrap(),
        authority.host(),
        Configuration {
            image_id: crate::config::IMAGE_ID,
            configuration_id: crate::config::CONFIGURATION_ID,
            friendly_label: "Fresh CPU factory",
            preparation: crate::config::PREPARATION_DELEGATION.as_ref(),
            execution: crate::config::EXECUTION_DELEGATION.as_ref(),
        },
    )
    .unwrap();
    authority.admit(&mut root);
    assert!(root.execution_parts().is_ok());
    println!("CPU_AUTHORITY_GATE public_root_spawn=true fresh_body=true current_member=true retained_input=false admission=false Start=false");
}
