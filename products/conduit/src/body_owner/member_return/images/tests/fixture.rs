use super::*;
pub struct Fixture {
    pub owner: Owner,
    pub root: PathBuf,
    pub prior: MembershipCredential,
    pub key: SpawnInvitationSecret,
    pub images: Vec<Vec<u8>>,
    pub projection: HostAdvertisementImageProjection,
}
impl Fixture {
    pub fn new(name: &str) -> Self {
        let (mut owner, root) = crate::durable_host::owner::member_return_owner_fixture();
        owner.prepare_current_member_carrier_receiver().unwrap();
        let invitation = owner.issue_invitation(&root, 60, None).unwrap();
        let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
        let mut actual = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: format!("host/{name}").into(),
            boot_id: format!("boot/{name}/1").into(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/image-member".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: actual.clone(),
            nonce: invitation.claim.nonce,
            signature: key
                .sign(&invitation.claim.signing_transcript(
                    &actual.host_id,
                    &actual.boot_id,
                    actual.offer_generation,
                ))
                .to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        let prior = owner
            .admit_invited(&root, request, actual.host_id.as_str())
            .unwrap()
            .credential;
        owner.member_leave(&root, &prior).unwrap();
        actual.boot_id = format!("boot/{name}/2").into();
        actual.offer_generation.0 += 1;
        let offer = conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
            capability_id: "fixture/sampler".into(), kind_id: "fixture/sample".into(), kind_contract_revision: "fixture/sample@1".into(),
            implementation: conduit_core::ImplementationOffer { execution_profile_id: "fixture/native@1".into(),
                implementation_id: "fixture/sampler@1".into(), artifact_id: "fixture/firmware@1".into() },
            inputs: vec![], outputs: vec![conduit_core::PortDescriptor {
                port_id: "sample".into(), value_kind: "value/count".into(), direction: conduit_core::PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: true }, abnormal_kind: None,
            }], host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
            limits: conduit_core::CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 64 },
        };
        let images = vec![stored_image(&serde_json::to_vec(&offer).unwrap())];
        let grants = (0..2)
            .map(|index| conduit_core::AuthorityGrant {
                grant_id: format!("fixture/grant-{index}").into(),
                contract_id: format!("fixture/authority-{index}@1").into(),
                host_call_contract_id: "fixture/call@1".into(),
                subject_kind: "fixture/sample".into(),
                host_id: actual.host_id.clone(),
                boot_id: actual.boot_id.clone(),
                capability_id: "fixture/sampler".into(),
            })
            .collect();
        let projection = HostAdvertisementImageProjection {
            context: actual,
            capability_images: images
                .iter()
                .map(|image| Sha256::digest(image).into())
                .collect(),
            authority_grants: grants,
        };
        Self {
            owner,
            root,
            prior,
            key,
            images,
            projection,
        }
    }
    pub fn signed(
        &self,
        projection: HostAdvertisementImageProjection,
    ) -> MemberReturnImagePresentation {
        let signature = self
            .key
            .sign(&member_return_image_transcript(&self.prior, &projection).unwrap())
            .to_vec();
        MemberReturnImagePresentation {
            kind: "Advertise".into(),
            schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
            credential: self.prior.clone(),
            projection,
            signature,
        }
    }
    pub fn presentation(&self) -> MemberReturnImagePresentation {
        self.signed(self.projection.clone())
    }
    pub fn begin(&mut self) -> Result<PartReturnChallenge, String> {
        let presentation = self.presentation();
        self.owner
            .fixture_begin_image_member_return(presentation, &self.images)
    }
    pub fn complete(
        &mut self,
        challenge: &PartReturnChallenge,
    ) -> Result<SignedPortableAdmissionReceipt, String> {
        let proof = self.proof(challenge);
        self.owner
            .fixture_complete_image_member_return(&self.root, &proof)
    }
    pub fn proof(&self, challenge: &PartReturnChallenge) -> PartReturnProof {
        PartReturnProof {
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            part_id: challenge.part_id.clone(),
            host_id: challenge.host_id.clone(),
            boot_id: challenge.boot_id.clone(),
            nonce: challenge.nonce,
            signature: self.key.sign(&challenge.signing_transcript()),
        }
    }
    pub fn grants(&self) -> Vec<conduit_core::AuthorityGrant> {
        self.projection
            .authority_grants
            .iter()
            .rev()
            .cloned()
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn stored_image(json: &[u8]) -> Vec<u8> {
    let length = u16::try_from(json.len()).unwrap();
    let mut image = b"CDZ1".to_vec();
    image.extend_from_slice(&(json.len() as u32).to_le_bytes());
    image.extend_from_slice(&[0x78, 0x01, 0x01]);
    image.extend_from_slice(&length.to_le_bytes());
    image.extend_from_slice(&(!length).to_le_bytes());
    image.extend_from_slice(json);
    let (mut a, mut b) = (1_u32, 0_u32);
    for byte in json {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    image.extend_from_slice(&((b << 16) | a).to_be_bytes());
    image
}
