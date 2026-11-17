use super::*;
pub struct Fixture {
    pub bootstrap: conduit_body::MemberBootstrap<'static>,
    pub actual: HostAdvertisement,
    pub member: SigningKey,
    pub member_public: [u8; 32],
    pub owner: SigningKey,
    pub prior: MembershipCredential,
    pub image: Vec<u8>,
    pub offer: CapabilityOffer,
}
impl Fixture {
    pub fn new() -> Self {
        let (bootstrap, actual, member, owner) = crate::membership_tests::fixture();
        let mut prior = crate::membership_tests::receipt(&bootstrap, &actual).credential;
        prior.boot_id = "fixture/prior-boot".into();
        let offer = conduit_core::capability_offer_from_parts! {
            semantic_contract: Default::default(), startup_parameters: vec![], shorthand: None,
            capability_id: "fixture/sample".into(), kind_id: "fixture/sample".into(),
            kind_contract_revision: "fixture/sample@1".into(),
            implementation: conduit_core::ImplementationOffer {
                execution_profile_id: "fixture/native@1".into(), implementation_id: "fixture/sample@1".into(),
                artifact_id: "fixture/image@1".into(),
            },
            inputs: vec![], outputs: vec![conduit_core::PortDescriptor {
                port_id: "sample".into(), value_kind: "value/count".into(),
                direction: conduit_core::PortDirection::Output,
                temporal: conduit_core::PortTemporal::Flow { closes: true }, abnormal_kind: None,
            }], host_calls: vec![], resource_requirements: vec![], authority_requirements: vec![],
            limits: conduit_core::CapabilityLimits { max_active_instances: 1, max_queue_items: 1, max_queue_bytes: 64 },
        };
        let image = stored_image(&serde_json::to_vec(&offer).unwrap());
        let member_public = member.verifying_key().to_bytes();
        Self {
            bootstrap,
            actual,
            member,
            member_public,
            owner,
            prior,
            image,
            offer,
        }
    }
    pub fn basis(&self) -> Basis<'_> {
        Basis {
            credential: &self.prior,
            expected_profile: &self.actual.profile,
            member_verifying_key: &self.member_public,
            owner: &self.bootstrap.owner,
        }
    }
    pub fn presentation(&self) -> MemberReturnImagePresentation {
        let projection = HostAdvertisementImageProjection {
            context: self.actual.clone(),
            capability_images: vec![Sha256::digest(&self.image).into()],
            authority_grants: vec![],
        };
        let mut presentation = MemberReturnImagePresentation {
            kind: "Advertise".into(),
            schema: MEMBER_RETURN_IMAGES_SCHEMA.into(),
            credential: self.prior.clone(),
            projection,
            signature: Vec::new(),
        };
        presentation.signature = self.sign_presentation(&presentation);
        presentation
    }
    pub fn sign_presentation(&self, presentation: &MemberReturnImagePresentation) -> Vec<u8> {
        self.member
            .sign(
                &member_return_image_transcript(&presentation.credential, &presentation.projection)
                    .unwrap(),
            )
            .to_bytes()
            .to_vec()
    }
    pub fn link(&self) -> Link<'_> {
        Link {
            member: &self.member,
            documents: [
                serde_json::to_vec(&self.presentation()).unwrap(),
                self.image.clone(),
            ]
            .into(),
            proof: None,
            sent: Vec::new(),
            received_documents: 0,
            bad_proof: false,
            fail_send: false,
        }
    }
    pub fn challenge(&self) -> PartReturnChallenge {
        PartReturnChallenge {
            admission_id: serde_json::from_str(r#""fixture/admission""#).unwrap(),
            body_id: self.prior.body_id.clone(),
            part_id: self.prior.part_id.clone(),
            host_id: self.actual.host_id.clone(),
            boot_id: self.actual.boot_id.clone(),
            offer_generation: self.actual.offer_generation,
            nonce: [9; 32],
            issued_at_millis: 1000,
            expires_at_millis: 2000,
        }
    }
    pub fn receipt(
        &self,
        projection: HostAdvertisementImageProjection,
    ) -> SignedPortableAdmissionReceipt {
        let mut current = self.prior.clone();
        current.boot_id = self.actual.boot_id.clone();
        current.credential_id = serde_json::from_str(r#""fixture/current-credential""#).unwrap();
        current.issued_at_millis = 1001;
        attest_member_return_image_acknowledgement(
            &MemberReturnImageAcknowledgement::new(current, projection),
            self.bootstrap.owner.clone(),
            self.member.verifying_key().to_bytes(),
            |t| self.owner.sign(t).to_bytes(),
        )
        .unwrap()
    }
}
// A valid zlib stored block avoids another compressor/test dependency. This is
// only a minimum executable image fixture, never a production encoding path.
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
pub struct Link<'a> {
    member: &'a SigningKey,
    pub documents: VecDeque<Vec<u8>>,
    proof: Option<Vec<u8>>,
    pub sent: Vec<Vec<u8>>,
    pub received_documents: usize,
    pub bad_proof: bool,
    pub fail_send: bool,
}
impl MessageTransport for Link<'_> {
    type Error = ();
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        self.sent.push(bytes.to_vec());
        if self.fail_send {
            return Err(());
        }
        if let Ok(challenge) = serde_json::from_slice::<PartReturnChallenge>(bytes) {
            let mut signature = self.member.sign(&challenge.signing_transcript()).to_bytes();
            if self.bad_proof {
                signature[0] ^= 1;
            }
            let proof = PartReturnProof {
                admission_id: challenge.admission_id,
                body_id: challenge.body_id,
                part_id: challenge.part_id,
                host_id: challenge.host_id,
                boot_id: challenge.boot_id,
                nonce: challenge.nonce,
                signature,
            };
            self.proof =
                Some(serde_json::to_vec(&MemberReturnProofDocument::encode(proof)).unwrap());
        }
        Ok(())
    }
    async fn receive<'a>(&mut self, output: &'a mut [u8]) -> Result<&'a [u8], ()> {
        let bytes = self.proof.take().ok_or(())?;
        let n = bytes.len();
        output.get_mut(..n).ok_or(())?.copy_from_slice(&bytes);
        Ok(&output[..n])
    }
}
impl DocumentTransport for Link<'_> {
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        self.received_documents += 1;
        let bytes = self.documents.pop_front().ok_or(())?;
        let digest = Sha256::digest(&bytes).into();
        let mut offset = 0;
        let mut frame = [0; 256];
        while offset < bytes.len() {
            let (length, next) =
                conduit_wire::encode_bounded_document_chunk(&mut frame, &bytes, offset, &digest)
                    .unwrap();
            document.push(&frame[..length]).map_err(|_| ())?;
            offset = next;
        }
        Ok(())
    }
}
