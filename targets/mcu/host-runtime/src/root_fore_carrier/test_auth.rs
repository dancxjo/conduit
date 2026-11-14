use conduit_body::*;
use conduit_core::{HostAdvertisement, OfferGeneration, PROTOCOL_VERSION};
use ed25519_dalek::SigningKey;
use std::vec;

pub struct Fixture {
    pub member: MembershipCredential,
    pub host: HostAdvertisement,
    pub bootstrap: MemberBootstrap<'static>,
    pub issuer: FederationPeer,
    pub key: SigningKey,
}
impl Fixture {
    pub fn new() -> Self {
        let key = SigningKey::from_bytes(&[9; 32]);
        let member: MembershipCredential = serde_json::from_value(serde_json::json!({
            "credential_id":"credential/1","body_id":"body/1","part_id":"part/1",
            "host_id":"host/1","boot_id":"boot/1","issued_at_millis":1,
        }))
        .unwrap();
        let issuer = FederationPeer {
            host_id: "issuer/1".into(),
            boot_id: "issuer/boot/1".into(),
            offer_generation: OfferGeneration(1),
            verifying_key: key.verifying_key().to_bytes(),
        };
        let bootstrap = MemberBootstrap {
            schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
            body_id: member.body_id.clone(),
            host_id: "host/1".into(),
            image_id: "image/1".into(),
            configuration_id: "config/1".into(),
            owner: issuer.clone(),
            admission: MemberBootstrapMaterial::Retained {
                credential: member.clone(),
                secret: [3; 32],
            },
        };
        Self {
            key,
            member,
            issuer,
            bootstrap,
            host: HostAdvertisement {
                declared_name: None,
                protocol_version: PROTOCOL_VERSION,
                host_id: "host/1".into(),
                boot_id: "boot/1".into(),
                offer_generation: OfferGeneration(1),
                profile: "profile/1".into(),
                bases: vec![],
                resources: vec![],
                capabilities: vec![],
                planner_capabilities: vec![],
            },
        }
    }
    pub fn context(&self) -> NativePreparationContext<'_> {
        NativePreparationContext {
            bootstrap: &self.bootstrap,
            member: &self.member,
            actual: &self.host,
            image_id: "image/1",
            configuration_id: "config/1",
            now_millis: 11,
        }
    }
}
