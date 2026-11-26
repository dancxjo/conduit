//! Actual generic planner, Body membership/session and route seal. Its admitted
//! observations and explicit Line references are CPU fixtures, not live membership.
use super::*;
use conduit_body::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, MembershipProofId, PartId,
};
use conduit_core::{
    process_owned_line_offer_with_limits, resource_offer, AuthorityGrantId, BaseImplementationId,
    CredentialReferenceId, HostAdvertisement, HostProfileId, LinkAuthorityReference,
    LinkCredentialReference, LinkLimits, OfferGeneration, FACE_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_face::{FaceBasis, FaceRole, FaceSubject, FaceText};
use serde::Serialize;

pub(super) struct Fixture {
    pub face: Face,
    pub route: RemoteOwnerMaskRouteSeal,
    browser: HostAdvertisement,
    owner: HostAdvertisement,
    session: BodyLifecycleSession,
}
fn host(name: &str) -> HostAdvertisement {
    let mut resources = vec![
        resource_offer("test/face", FACE_RESOURCE_CLASS, 1),
        resource_offer(
            profile::MEMORY_POOL,
            profile::MEMORY_CLASS,
            profile::PREPARATION_BYTES * profile::PREPARATION_SLOTS as u32,
        ),
    ];
    resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: format!("host/refusal-{name}").into(),
        boot_id: format!("boot/refusal-{name}").into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("test/full-refusal"),
        bases: vec![],
        resources,
        capabilities: vec![profile::offer()],
        planner_capabilities: vec![],
    }
}
impl Fixture {
    pub fn new() -> Self {
        abi::conduit_browser_owner_face_clear();
        super::super::super::reset_full_interface().unwrap();
        pool::reset().unwrap();
        let browser = host("browser");
        let owner = host("owner");
        pool::prepare(&browser.host_id, &browser.boot_id).unwrap();
        super::super::super::prepare_full_interface().unwrap();
        let canonical = serde_json::to_vec(&browser).unwrap();
        pool::capture_advertisement(&browser, &canonical).unwrap();
        pool::freeze().unwrap();
        let body = Body::born(
            "source/full-refusal".into(),
            "checked/full-refusal".into(),
            1,
            "sign/born-full-refusal".into(),
        )
        .unwrap();
        let id = body.body_id.clone();
        let mut membership = BodyMembership::new(id.clone()).unwrap();
        let mut evidence =
            BodyBiographyEvidence::born(body, membership.clone(), "Refusal fixture".into())
                .unwrap();
        let part = PartId::bind(&id, browser.host_id.as_str(), 1).unwrap();
        let proof = MembershipProofId::bind("test/full-refusal").unwrap();
        let admitted = membership
            .admit(
                &id,
                membership.revision,
                part.clone(),
                proof.clone(),
                "sign/refusal-admit".into(),
            )
            .unwrap();
        let present = membership
            .observe_present(
                &id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: browser.host_id.clone(),
                    boot_id: browser.boot_id.clone(),
                    offer_generation: browser.offer_generation,
                    proof_id: proof,
                    sequence: 1,
                },
                "sign/refusal-present".into(),
            )
            .unwrap();
        evidence
            .append_membership_events(membership, &[(admitted, 2), (present, 3)])
            .unwrap();
        let session = BodyLifecycleSession::open(evidence).unwrap();
        let face = Face::new(
            3,
            FaceBasis {
                body_id: Some(id),
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![FaceSubject {
                identity: "reading".into(),
                role: FaceRole::Plot,
                name: "Original reading".into(),
            }],
            vec![],
            vec![],
            vec![FaceText {
                subject: "reading".into(),
                text: "Complete checked fixture reading".into(),
            }],
        )
        .unwrap();
        let route = seal(&session, &face, &owner, &browser);
        Self {
            face,
            route,
            browser,
            owner,
            session,
        }
    }
    pub fn legacy_frame(&self) -> Vec<u8> {
        let mut browser = self.browser.clone();
        browser.capabilities = vec![conduit_browser_mask_offer::offer()];
        let route = seal_profile(&self.session, &self.face, &self.owner, &browser, false);
        self.frame(&self.face, &route)
    }
    pub fn basis(&self) -> serde_json::Value {
        serde_json::json!({"body_id":self.face.basis.body_id.as_ref().unwrap(),
            "host_id":self.browser.host_id,"boot_id":self.browser.boot_id})
    }
    pub fn frame(&self, face: &Face, route: &RemoteOwnerMaskRouteSeal) -> Vec<u8> {
        #[derive(Serialize)]
        struct Snapshot<'a> {
            outcome: &'static str,
            schema: &'static str,
            face: &'a Face,
            interactions_admitted: bool,
            route: &'a RemoteOwnerMaskRouteSeal,
        }
        #[derive(Serialize)]
        struct Frame<'a> {
            kind: &'static str,
            protocol: u16,
            response: Snapshot<'a>,
        }
        serde_json::to_vec(&Frame {
            kind: "face-snapshot-response",
            protocol: 1,
            response: Snapshot {
                outcome: "snapshot",
                schema: OWNER_FACE_RESPONSE_SCHEMA,
                face,
                interactions_admitted: false,
                route,
            },
        })
        .unwrap()
    }
    pub fn submit(&self, basis: &serde_json::Value, frame: &[u8]) -> i32 {
        let basis = serde_json::to_vec(basis).unwrap();
        let mut bytes = basis.clone();
        bytes.extend_from_slice(frame);
        input(&bytes);
        abi::conduit_browser_owner_face_prepare(basis.len(), frame.len())
    }
    pub fn install(&self) -> serde_json::Value {
        assert_eq!(
            self.submit(&self.basis(), &self.frame(&self.face, &self.route)),
            0
        );
        output_json()
    }
    pub fn acknowledge(&self, view: &serde_json::Value) {
        let bytes = serde_json::to_vec(
            &serde_json::json!({"show_id":view["show_id"],"face_id":view["face_id"],
            "face_revision":view["face_revision"]}),
        )
        .unwrap();
        input(&bytes);
        assert_eq!(abi::conduit_browser_owner_face_ack(bytes.len()), 0);
    }
    pub fn available(&self) -> Vec<u8> {
        let view = self.install();
        self.acknowledge(&view);
        self.current()
    }
    pub fn current(&self) -> Vec<u8> {
        assert_eq!(abi::conduit_browser_owner_face_canonical_snapshot(), 0);
        output()
    }
    pub fn unsupported_face(&self) -> (Face, RemoteOwnerMaskRouteSeal) {
        let face = Face::new_with_semantics(
            4,
            self.face.basis.clone(),
            self.face.subjects.clone(),
            vec![],
            vec![],
            self.face.text.clone(),
            vec![conduit_face::FaceAction {
                identity: "edit".into(),
                intent: "value/change@1".into(),
                target: "reading".into(),
                name: "Edit".into(),
                arguments: vec![conduit_face::FaceActionArgument::text(
                    "text".into(),
                    "Text".into(),
                    0,
                    8,
                )
                .unwrap()],
                disclosure: conduit_face::FaceDisclosureLevel::Primary,
                availability: conduit_face::FaceActionAvailability::Available,
            }],
            vec![],
        )
        .unwrap();
        let route = seal(&self.session, &face, &self.owner, &self.browser);
        (face, route)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        abi::conduit_browser_owner_face_clear();
        super::super::super::reset_full_interface().unwrap();
        pool::reset().unwrap();
    }
}
fn seal(
    session: &BodyLifecycleSession,
    face: &Face,
    owner: &HostAdvertisement,
    browser: &HostAdvertisement,
) -> RemoteOwnerMaskRouteSeal {
    seal_profile(session, face, owner, browser, true)
}
fn seal_profile(
    session: &BodyLifecycleSession,
    face: &Face,
    owner: &HostAdvertisement,
    browser: &HostAdvertisement,
    full: bool,
) -> RemoteOwnerMaskRouteSeal {
    let descriptor = if full {
        &conduit_host_browser_make::OWNER_FACE_FULL_LINE
    } else {
        &conduit_host_browser_make::BROWSER_LINE_REALIZATIONS[0]
    };

    let limits = LinkLimits {
        maximum_in_flight_items: descriptor.maximum_in_flight_items,
        maximum_payload_bytes: descriptor.maximum_payload_bytes,
        maximum_buffered_bytes: descriptor.maximum_buffered_bytes,
        maximum_frame_bytes: descriptor.maximum_frame_bytes,
    };
    let line = |name: &str, source: &HostAdvertisement, sink: &HostAdvertisement| {
        let mut line = process_owned_line_offer_with_limits(
            name,
            &format!("binding/{name}"),
            BaseImplementationId::from(descriptor.base_implementation_id),
            "test/refusal-base",
            source,
            sink,
            limits,
        );
        // Explicit CPU fixture references, not possession of a real endpoint
        // grant or proof that a remote peer authenticated this test process.
        line.binding.credential = LinkCredentialReference::Opaque(CredentialReferenceId::from(
            "credential/refusal-fixture",
        ));
        line.binding.authority =
            LinkAuthorityReference::Grant(AuthorityGrantId::from("grant/refusal-fixture"));
        line.contract = descriptor.contract;
        line
    };
    let face_line = line("line/refusal-face", owner, browser);
    let returned = line("line/refusal-return", browser, owner);
    let selected = if full {
        profile::planning_host(browser).unwrap()
    } else {
        browser.clone()
    };
    let planned = conduit_browser_mask_offer::planned_owner_face_show_mask(
        &selected,
        owner,
        &face_line,
        &returned,
        conduit_browser_mask_offer::MASK_SOURCE,
        "browser-graphical",
    )
    .unwrap();
    RemoteOwnerMaskRouteSeal::seal_current_hosted_presentation(
        session, face, owner, browser, &planned, &face_line, &returned,
    )
    .unwrap()
}
pub(super) fn input(bytes: &[u8]) {
    assert!(bytes.len() <= abi::conduit_browser_owner_face_input_capacity());
    let pointer = abi::conduit_browser_owner_face_input_ptr() as *mut u8;
    // SAFETY: this thread owns the ABI's fixed initialized input buffer. No
    // RefCell borrow or pointer alias survives this copy or the next ABI call.
    unsafe {
        std::slice::from_raw_parts_mut(pointer, bytes.len()).copy_from_slice(bytes);
    }
}
pub(super) fn output() -> Vec<u8> {
    let length = abi::conduit_browser_owner_face_output_len();
    assert!(length <= abi::conduit_browser_owner_face_output_capacity());
    let pointer = abi::conduit_browser_owner_face_output_ptr() as *const u8;
    // SAFETY: the ABI reports the initialized live output extent, copied before
    // any subsequent operation can mutate its backing allocation.
    unsafe { std::slice::from_raw_parts(pointer, length).to_vec() }
}
fn output_json() -> serde_json::Value {
    serde_json::from_slice(&output()).unwrap()
}
