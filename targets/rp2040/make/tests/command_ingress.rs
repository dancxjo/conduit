//! Actual Root handler, with fake time/radio and an owner signing its challenge.
extern crate self as embassy_rp;
extern crate self as embassy_time;
use conduit_body::*;
use conduit_core as c;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::{cell::Cell, future::Future, ops::Add, task::Poll};

#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod probe;
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;
const LIMIT: usize = 4096;
mod config {
    pub const RESIDENT_PREPARATION_STORAGE_BYTES: usize = 16384;
    pub const RESIDENT_PREPARATION_MAXIMA: conduit_core::AssignedPlanMaxima =
        conduit_core::AssignedPlanMaxima {
            encoded_bytes: 8192,
            runtime_state_bytes: 8192,
            counts: [32; conduit_core::ASSIGNED_PLAN_COUNT_KINDS],
        };
    pub const IMAGE_ID: &str = "fixture/image";
    pub const CONFIGURATION_ID: &str = "fixture/config";
    pub const DIAGNOSTIC_HTTP_BUFFER_LIMIT: u64 = 8192;
    pub const PREPARATION_DELEGATION: Option<conduit_body::HostPreparationDelegation> =
        Some(conduit_body::HostPreparationDelegation {
            authorizer: conduit_body::HostPreparationAuthorizer::SeedOwner,
            maximum_assignment_bytes: 8192,
            maximum_runtime_state_bytes: 8192,
        });
}
mod resident_preparation {
    pub enum ValidationRefusal {
        Unavailable,
        InvalidAssignment,
        #[allow(dead_code)]
        Storage(conduit_kernel::scheduler::KernelPreparationError),
    }
    pub struct AttemptStorage;
    impl Drop for AttemptStorage {
        fn drop(&mut self) {}
    }
    pub struct Inventory;
    impl Drop for Inventory {
        fn drop(&mut self) {}
    }
    pub fn inventory() -> Result<Inventory, ()> {
        Ok(Inventory)
    }
    pub fn prepare_workset_storage(
        _: &mut Inventory,
        _: Option<&conduit_body::HostPreparationDelegation>,
        _: conduit_body::NativePreparationContext<'_>,
        command: conduit_body::AttributedHostCommand<'_, '_>,
        _: conduit_core::AssignedPlanMaxima,
        _: usize,
    ) -> Result<AttemptStorage, ValidationRefusal> {
        if command.payload().first() == Some(&0x46) {
            Err(ValidationRefusal::InvalidAssignment)
        } else {
            Err(ValidationRefusal::Unavailable)
        }
    }
}
thread_local! { static NOW: Cell<u64> = const { Cell::new(100) }; }
#[derive(Clone, Copy)]
pub struct Instant(u64);
pub struct Duration(u64);
impl Instant {
    pub fn now() -> Self {
        Self(NOW.with(Cell::get))
    }
    pub fn as_millis(self) -> u64 {
        self.0
    }
}
impl Duration {
    pub fn from_millis(ms: u64) -> Self {
        Self(ms)
    }
}
impl Add<Duration> for Instant {
    type Output = Self;
    fn add(self, rhs: Duration) -> Self {
        Self(self.0 + rhs.0)
    }
}
#[derive(Debug)]
pub struct TimeoutError;
pub async fn with_deadline<F: Future>(at: Instant, future: F) -> Result<F::Output, TimeoutError> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| {
        if Instant::now().0 >= at.0 {
            return Poll::Ready(Err(TimeoutError));
        }
        match future.as_mut().poll(cx) {
            Poll::Ready(value) => Poll::Ready(Ok(value)),
            Poll::Pending if Instant::now().0 >= at.0 => Poll::Ready(Err(TimeoutError)),
            Poll::Pending => Poll::Pending,
        }
    })
    .await
}
pub mod clocks {
    pub struct RoscRng;
    impl RoscRng {
        pub fn next_u64(&mut self) -> u64 {
            0x12345678abcdef01
        }
    }
}
struct Membership {
    seed: MemberBootstrap<'static>,
    receipt: Option<MembershipCredential>,
    resources: c::ResourceAdmissionOwner,
    freshness: u64,
}
#[path = "../../firmware/host/src/membership_preparation.rs"]
mod preparation;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Valid,
    InvalidAssignment,
    BadSignature,
    Replay,
    BadDigest,
    Interrupted,
    Deadline,
    Pending,
}
struct Link {
    mode: Mode,
    payload: Vec<u8>,
    frames: Vec<Vec<u8>>,
    signature: [u8; 64],
    response: [u8; 256],
    response_len: usize,
    challenged: bool,
    nonce: [u8; 32],
    reads: usize,
}
impl Link {
    fn new(mode: Mode, length: usize) -> Self {
        let payload = vec![
            if mode == Mode::InvalidAssignment {
                0x46
            } else {
                0x45
            };
            length
        ];
        let digest: [u8; 32] = Sha256::digest(&payload).into();
        let mut frames = Vec::new();
        let mut offset = 0;
        while offset < length {
            let mut frame = vec![0u8; 128];
            let (used, next) =
                conduit_wire::encode_bounded_document_chunk(&mut frame, &payload, offset, &digest)
                    .unwrap();
            frame.truncate(used);
            if mode == Mode::BadDigest {
                frame[12] ^= 1;
            }
            frames.push(frame);
            offset = next;
        }
        Self {
            mode,
            payload,
            frames,
            signature: [0; 64],
            response: [0; 256],
            response_len: 0,
            challenged: false,
            nonce: [0; 32],
            reads: 0,
        }
    }
    async fn send(&mut self, bytes: &[u8]) -> Result<(), ()> {
        if bytes.windows(10).any(|b| b == b"\"body_id\":") {
            let (challenge, used): (HostCommandChallenge<'_>, _) =
                serde_json_core::from_slice(bytes).map_err(|_| ())?;
            assert_eq!(used, bytes.len());
            assert_eq!(challenge.maximum_payload_bytes, LIMIT as u32);
            assert_eq!(challenge.payload_contract, HOST_PREPARED_WORKSET_CONTRACT);
            self.challenged = true;
            self.nonce = challenge.nonce;
            if self.payload.len() <= LIMIT && self.mode != Mode::Replay {
                self.signature = SigningKey::from_bytes(&[9; 32])
                    .sign(&host_command_attribution_transcript(&challenge, &self.payload).unwrap())
                    .to_bytes();
                if self.mode == Mode::BadSignature {
                    self.signature[0] ^= 1;
                }
            }
        } else {
            self.response[..bytes.len()].copy_from_slice(bytes);
            self.response_len = bytes.len();
        }
        Ok(())
    }
    async fn receive_document(
        &mut self,
        document: &mut conduit_wire::BorrowedBoundedDocumentAssembly<'_>,
    ) -> Result<(), ()> {
        self.reads += 1;
        if self.mode == Mode::Interrupted {
            return Err(());
        }
        if matches!(self.mode, Mode::Pending | Mode::Deadline) {
            document.push(&self.frames[0]).map_err(|_| ())?;
            if self.mode == Mode::Deadline {
                NOW.with(|n| n.set(10100));
            }
            std::future::pending::<()>().await;
        }
        for frame in &self.frames {
            if document.push(frame).map_err(|_| ())? {
                return Ok(());
            }
        }
        Err(())
    }
    async fn receive<'a>(&mut self, bytes: &'a mut [u8]) -> Result<&'a [u8], ()> {
        self.reads += 1;
        bytes[..64].copy_from_slice(&self.signature);
        Ok(&bytes[..64])
    }
}
fn member() -> Membership {
    let actual = c::HostAdvertisement {
        declared_name: None,
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/host".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: config::CONFIGURATION_ID.into(),
        bases: vec![],
        resources: vec![c::resource_offer(
            "fixture/work",
            c::COOPERATIVE_STEP_WORK_CLASS_ID,
            1,
        )],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let receipt: MembershipCredential=serde_json::from_value(serde_json::json!({
        "credential_id":"fixture/credential","body_id":"fixture/body","part_id":"fixture/part","host_id":actual.host_id,"boot_id":actual.boot_id,"issued_at_millis":1
    })).unwrap();
    let seed = MemberBootstrap {
        schema: MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: receipt.body_id.clone(),
        host_id: actual.host_id.as_str().into(),
        image_id: config::IMAGE_ID.into(),
        configuration_id: config::CONFIGURATION_ID.into(),
        owner: FederationPeer {
            host_id: "fixture/owner".into(),
            boot_id: "fixture/owner-boot".into(),
            offer_generation: c::OfferGeneration(1),
            verifying_key: SigningKey::from_bytes(&[9; 32]).verifying_key().to_bytes(),
        },
        admission: MemberBootstrapMaterial::Retained {
            credential: receipt.clone(),
            secret: [3; 32],
        },
    };
    Membership {
        seed,
        receipt: Some(receipt),
        resources: c::ResourceAdmissionOwner::new(actual),
        freshness: 0,
    }
}
fn run<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match future.as_mut().poll(&mut context) {
        Poll::Ready(v) => v,
        Poll::Pending => panic!("bounded fixture must finish"),
    }
}
#[test]
fn actual_root_ingress_authenticates_without_another_document_allocation_or_start() {
    for mode in [
        Mode::Valid,
        Mode::InvalidAssignment,
        Mode::BadSignature,
        Mode::BadDigest,
        Mode::Interrupted,
        Mode::Deadline,
    ] {
        NOW.with(|n| n.set(100));
        let mut member = member();
        let before = member.resources.clone();
        let mut link = Link::new(mode, 1000);
        let mut workspace = [0x44; LIMIT];
        let future = member.serve_preparation(&mut link, &mut workspace);
        assert!(std::mem::size_of_val(&future) < 2048);
        let (result, cost) = probe::observe(|| run(future));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert!(link.challenged);
        assert!(workspace.iter().all(|b| *b == 0));
        assert_eq!(member.resources, before);
        assert_eq!(result.is_ok(), mode == Mode::Valid);
        if mode == Mode::Valid {
            assert_eq!(
                &link.response[..link.response_len],
                br#"{"phase":"attributed","preparation":"inventory-unavailable"}"#
            );
        } else {
            assert_eq!(link.response_len, 0);
        }
    }
}
#[test]
fn current_membership_and_actual_receive_extent_are_required() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut link = Link::new(Mode::Valid, 1000);
    let mut workspace = [0; LIMIT];
    member.receipt = None;
    assert!(run(member.serve_preparation(&mut link, &mut workspace)).is_ok());
    assert!(!link.challenged);
    assert_eq!(link.reads, 0);
    assert!(std::str::from_utf8(&link.response[..link.response_len])
        .unwrap()
        .contains("membership-not-current"));
    let mut member = crate::member();
    let mut link = Link::new(Mode::Valid, LIMIT + 1);
    assert!(run(member.serve_preparation(&mut link, &mut workspace)).is_err());
    assert!(member.resources.admissions().is_empty());
    assert!(workspace.iter().all(|b| *b == 0));
}
#[test]
fn cancellation_drops_the_challenge_and_returns_the_exclusive_workspace() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut link = Link::new(Mode::Pending, 1000);
    let mut workspace = [0; LIMIT];
    let mut future = Box::pin(member.serve_preparation(&mut link, &mut workspace));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    drop(future);
    // The borrow is released; no admission or active execution was created.
    assert!(member.resources.admissions().is_empty());
    assert!(link.challenged);
    workspace.fill(0);
}

#[test]
fn fresh_attempt_cannot_reuse_an_old_signature_even_if_the_rng_repeats() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut workspace = [0; LIMIT];
    let mut first = Link::new(Mode::Valid, 1000);
    assert!(run(member.serve_preparation(&mut first, &mut workspace)).is_ok());
    let mut second = Link::new(Mode::Replay, 1000);
    second.signature = first.signature;
    assert!(run(member.serve_preparation(&mut second, &mut workspace)).is_err());
    assert_ne!(first.nonce, second.nonce);
    assert_eq!(member.freshness, 2);
    assert!(member.resources.admissions().is_empty());
    assert_eq!(second.response_len, 0);
}

#[test]
fn busy_root_does_not_consume_the_document_or_issue_another_attempt() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let ad = member.resources.host().clone();
    let requirement = c::ResourceRequirement {
        class_id: ad.resources[0].class_id.clone(),
        units: 1,
        protected_role: None,
        compute: None,
        content: None,
    };
    let observed = c::ResourceObservation {
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        offer_generation: ad.offer_generation,
        pool_id: ad.resources[0].pool_id.clone(),
        class_id: ad.resources[0].class_id.clone(),
        health: c::ResourceHealth::Ready,
        unreserved_units: 1,
        utilized_units: 0,
        sign_id: "fixture/ready".into(),
    };
    member
        .resources
        .admit(
            c::ResourceAdmissionRequest {
                plan_id: "fixture/retained-plan".into(),
                placement_id: "fixture/retained-placement".into(),
                host_id: ad.host_id,
                boot_id: ad.boot_id,
                offer_generation: ad.offer_generation,
                items: vec![c::ResourceAdmissionItem {
                    binding: c::ResourceBinding {
                        pool_id: ad.resources[0].pool_id.clone(),
                        class_id: requirement.class_id.clone(),
                        units: 1,
                        protected: None,
                        compute: None,
                        content: None,
                    },
                    requirement,
                }],
            },
            &[observed],
        )
        .unwrap();
    let before = member.resources.clone();
    let mut link = Link::new(Mode::Valid, 1000);
    let mut workspace = [0; LIMIT];
    let (result, cost) =
        probe::observe(|| run(member.serve_preparation(&mut link, &mut workspace)));
    assert!(result.is_ok());
    assert_eq!(cost.allocations, 0);
    assert!(!link.challenged);
    assert_eq!(link.reads, 0);
    assert_eq!(member.freshness, 0);
    assert_eq!(member.resources, before);
    assert!(std::str::from_utf8(&link.response[..link.response_len])
        .unwrap()
        .contains("awaiting-native-retirement"));
}
