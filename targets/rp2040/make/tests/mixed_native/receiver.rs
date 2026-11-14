use conduit_body::*;
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
use std::cell::Cell;
pub(crate) struct Receiver {
    member: MembershipCredential,
    pub actual: c::HostAdvertisement,
    bootstrap: MemberBootstrap<'static>,
    key: SigningKey,
    nonce: Cell<u8>,
}
impl Receiver {
    pub fn new(scope: PreparedBodyFragment<'_>) -> Self {
        let f = scope.fragment();
        let key = SigningKey::from_bytes(&[9; 32]);
        let member: MembershipCredential=serde_json::from_value(serde_json::json!({
            "credential_id":"fixture/credential", "body_id":scope.plan().body_id,"part_id":"fixture/part",
            "host_id":f.host_id,"boot_id":f.boot_id,"issued_at_millis":1,
        })).unwrap();
        let owner = FederationPeer {
            host_id: "fixture/issuer".into(),
            boot_id: "fixture/issuer/boot".into(),
            offer_generation: c::OfferGeneration(1),
            verifying_key: key.verifying_key().to_bytes(),
        };
        let bootstrap = MemberBootstrap {
            schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
            body_id: member.body_id.clone(),
            host_id: f.host_id.as_str().into(),
            image_id: "fixture/image".into(),
            configuration_id: "fixture/config".into(),
            owner,
            admission: MemberBootstrapMaterial::Retained {
                credential: member.clone(),
                secret: [3; 32],
            },
        };
        Self {
            member,
            bootstrap,
            key,
            nonce: Cell::new(1),
            actual: c::HostAdvertisement {
                declared_name: None,
                protocol_version: c::PROTOCOL_VERSION,
                host_id: f.host_id.clone(),
                boot_id: f.boot_id.clone(),
                offer_generation: f.offer_generation,
                profile: "fixture/config".into(),
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
            actual: &self.actual,
            image_id: "fixture/image",
            configuration_id: "fixture/config",
            now_millis: 11,
        }
    }
    fn command<'a>(&'a self, bytes: &'a [u8]) -> AttributedHostCommand<'a, 'a> {
        self.command_with_contract(bytes, HOST_PREPARED_FRAGMENT_CONTRACT)
    }
    pub fn command_with_contract<'a>(
        &'a self,
        bytes: &'a [u8],
        contract: &'a str,
    ) -> AttributedHostCommand<'a, 'a> {
        let nonce = self.nonce.get();
        self.nonce.set(nonce.checked_add(1).unwrap());
        let slot = IssuedHostCommandChallenge::issue(
            &self.member,
            &self.actual,
            &self.bootstrap.owner,
            HostCommandReceiveBounds {
                payload_contract: contract,
                maximum_payload_bytes: 4096,
                lifetime_millis: 10,
            },
            [nonce; 32],
            10,
        )
        .unwrap();
        let signature = self
            .key
            .sign(&host_command_attribution_transcript(slot.challenge(), bytes).unwrap())
            .to_bytes();
        slot.authenticate(bytes, &signature, 11).unwrap()
    }
}
