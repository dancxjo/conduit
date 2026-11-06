//! Actual Root handler, with fake time/radio and an owner signing its challenge.
extern crate alloc;
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
const BUILD_ID: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
mod transport {
    pub(crate) type Link = crate::Link;
}
#[path = "../../firmware/host/src/bootsel.rs"]
mod bootsel;
thread_local! { static RESET: Cell<bool> = const { Cell::new(false) }; }
pub mod rom_data {
    pub fn reset_to_usb_boot(_: u32, _: u32) {
        crate::RESET.with(|flag| flag.set(true));
    }
}
pub struct Timer;
impl Timer {
    pub async fn after_millis(_: u64) {}
}
mod config {
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
    pub struct Inventory;
    impl Drop for Inventory {
        fn drop(&mut self) {}
    }
    thread_local! {
        pub static BUSY: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
        pub static WITHDRAWN: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
    }
    impl Inventory {
        pub fn offers(&self) -> [Option<&[u8]>; 1] {
            [if WITHDRAWN.with(core::cell::Cell::get) {
                None
            } else {
                Some(b"ready")
            }]
        }
    }
    pub fn inventory() -> Result<Inventory, ()> {
        if BUSY.with(core::cell::Cell::get) {
            Err(())
        } else {
            Ok(Inventory)
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
struct MembershipState {
    seed: MemberBootstrap<'static>,
    receipt: Option<MembershipCredential>,
    resources: c::ResourceAdmissionOwner,
    freshness: u64,
}
#[path = "../../firmware/host/src/membership_preparation.rs"]
mod preparation;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    ValidAttribution,
    OpaqueInput,
    BadSignature,
    Replay,
    BadDigest,
    Interrupted,
    Deadline,
    Pending,
    Receipt,
    ReceiptPending,
    ReceiptLostResponse,
    ReceiptDeadline,
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
    control: Option<std::collections::VecDeque<Vec<u8>>>,
}
impl Link {
    fn new(mode: Mode, length: usize) -> Self {
        let payload = vec![
            if matches!(
                mode,
                Mode::Receipt
                    | Mode::ReceiptPending
                    | Mode::ReceiptLostResponse
                    | Mode::ReceiptDeadline
            ) {
                0x52
            } else if mode == Mode::OpaqueInput {
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
            control: None,
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
            if self.mode == Mode::ReceiptDeadline {
                NOW.with(|n| n.set(10100));
                core::future::pending::<()>().await;
            }
            if self.mode == Mode::ReceiptPending {
                core::future::pending::<()>().await;
            }
            if self.mode == Mode::ReceiptLostResponse {
                return Err(());
            }
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
        if self.control.is_some() {
            loop {
                // The real decoder lends its fixed frame; the fixture's owned
                // Vec and temporary copy target must not inflate that future.
                let length = {
                    let frame = self.control.as_mut().unwrap().pop_front().ok_or(())?;
                    let length = frame.len();
                    bytes.get_mut(..length).ok_or(())?.copy_from_slice(&frame);
                    length
                };
                if bootsel::handle(self, &bytes[..length]).await? {
                    continue;
                }
                return Ok(&bytes[..length]);
            }
        }
        bytes[..64].copy_from_slice(&self.signature);
        Ok(&bytes[..64])
    }
}
fn member() -> MembershipState {
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
    MembershipState {
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
        Mode::ValidAttribution,
        Mode::OpaqueInput,
        Mode::BadSignature,
        Mode::BadDigest,
        Mode::Interrupted,
        Mode::Deadline,
    ] {
        NOW.with(|n| n.set(100));
        let mut member = member();
        let before = member.resources.clone();
        let mut link = Link::new(mode, 1000);
        let mut workspace = HostCommandInput::<LIMIT>::new();
        workspace.writable().unwrap().fill(0x44);
        let future = member.serve_preparation(&mut link, &mut workspace);
        assert!(
            std::mem::size_of_val(&future) < 2048,
            "ingress frame {} bytes",
            std::mem::size_of_val(&future)
        );
        let (result, cost) = probe::observe(|| run(future));
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        assert!(link.challenged);
        assert_eq!(member.resources, before);
        let retained = matches!(mode, Mode::ValidAttribution | Mode::OpaqueInput);
        assert_eq!(result.is_ok(), retained);
        assert_eq!(workspace.is_retained(), retained);
        if retained {
            assert_eq!(workspace.payload().unwrap(), link.payload);
            let report: serde_json::Value =
                serde_json::from_slice(&link.response[..link.response_len]).unwrap();
            assert_eq!(report["phase"], "input-retained");
            assert_eq!(report["resources"], "not-admitted");
            assert_eq!(report["start"], "not-started");
        } else {
            assert!(workspace.writable().unwrap().iter().all(|b| *b == 0));
            assert_eq!(link.response_len, 0);
        }
    }
}
#[test]
fn current_membership_and_actual_receive_extent_are_required() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut link = Link::new(Mode::ValidAttribution, 1000);
    let mut workspace = HostCommandInput::<LIMIT>::new();
    member.receipt = None;
    assert!(run(member.serve_preparation(&mut link, &mut workspace)).is_ok());
    assert!(!link.challenged);
    assert_eq!(link.reads, 0);
    assert!(std::str::from_utf8(&link.response[..link.response_len])
        .unwrap()
        .contains("membership-not-current"));
    let mut member = crate::member();
    let mut link = Link::new(Mode::ValidAttribution, LIMIT + 1);
    assert!(run(member.serve_preparation(&mut link, &mut workspace)).is_err());
    assert!(member.resources.admissions().is_empty());
    assert!(workspace.writable().unwrap().iter().all(|b| *b == 0));
}
#[test]
fn cancellation_drops_the_challenge_and_returns_the_exclusive_workspace() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut link = Link::new(Mode::Pending, 1000);
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut future = Box::pin(member.serve_preparation(&mut link, &mut workspace));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(future.as_mut().poll(&mut context).is_pending());
    drop(future);
    // The borrow is released; no admission or active execution was created.
    assert!(member.resources.admissions().is_empty());
    assert!(link.challenged);
    workspace.writable().unwrap().fill(0);
}

#[test]
fn fresh_attempt_cannot_reuse_an_old_signature_even_if_the_rng_repeats() {
    NOW.with(|n| n.set(100));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut first = Link::new(Mode::ValidAttribution, 1000);
    assert!(run(member.serve_preparation(&mut first, &mut workspace)).is_ok());
    // This courier-only test owns no kernel or reservations. Explicitly retire
    // the unstarted input to test a fresh challenge at the same clock value.
    workspace.retire();
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
    let mut link = Link::new(Mode::ValidAttribution, 1000);
    let mut workspace = HostCommandInput::<LIMIT>::new();
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

#[test]
fn actual_handler_retains_only_input_after_response_loss_or_cancellation() {
    for mode in [
        Mode::Receipt,
        Mode::ReceiptLostResponse,
        Mode::ReceiptPending,
        Mode::ReceiptDeadline,
    ] {
        NOW.with(|n| n.set(100));
        let mut member = member();
        let before = member.resources.clone();
        let mut link = Link::new(mode, 1000);
        let mut workspace = HostCommandInput::<LIMIT>::new();
        let mut future = Box::pin(member.serve_preparation(&mut link, &mut workspace));
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        let result = future.as_mut().poll(&mut cx);
        assert_eq!(result.is_pending(), mode == Mode::ReceiptPending);
        if mode == Mode::Receipt {
            assert!(matches!(result, Poll::Ready(Ok(()))));
        }
        if matches!(mode, Mode::ReceiptLostResponse | Mode::ReceiptDeadline) {
            assert!(matches!(result, Poll::Ready(Err(()))));
        }
        drop(future);
        assert_eq!(member.resources, before);
        assert_eq!(workspace.payload().unwrap(), link.payload);
        let report: serde_json::Value =
            serde_json::from_slice(&link.response[..link.response_len]).unwrap();
        assert_eq!(report["custody"], "ram");
        assert_eq!(
            report["payload_sha256"],
            format!("{:x}", Sha256::digest(&link.payload))
        );
        assert_eq!(
            workspace.writable().err(),
            Some(HostCommandRetentionRefusal::Occupied)
        );
        let original_nonce_sequence = member.freshness;
        let mut retry = Link::new(Mode::ValidAttribution, 1000);
        assert!(run(member.serve_preparation(&mut retry, &mut workspace)).is_ok());
        if mode == Mode::ReceiptDeadline {
            // The original deadline ended with no preparation or reservation.
            // The next request receives a fresh challenge, not a lease
            // renewal or another acknowledgment of expired input.
            assert!(retry.challenged);
            assert_eq!(member.freshness, original_nonce_sequence + 1);
            assert!(workspace.is_retained());
        } else {
            assert!(!retry.challenged);
            assert_eq!(retry.reads, 0);
            assert_eq!(member.freshness, original_nonce_sequence);
            assert_eq!(workspace.payload().unwrap(), link.payload);
            let repeated: serde_json::Value =
                serde_json::from_slice(&retry.response[..retry.response_len]).unwrap();
            assert_eq!(repeated["payload_sha256"], report["payload_sha256"]);
        }
        assert!(std::str::from_utf8(&link.response[..link.response_len])
            .unwrap()
            .contains("input-retained"));
        assert!(std::str::from_utf8(&link.response[..link.response_len])
            .unwrap()
            .contains("not-started"));
    }
}

#[test]
fn occupied_input_keeps_build_bound_recovery_and_sealed_bytes() {
    NOW.with(|n| n.set(100));
    RESET.with(|flag| flag.set(false));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut prepared = Link::new(Mode::Receipt, 1000);
    assert!(run(member.serve_preparation(&mut prepared, &mut workspace)).is_ok());
    let digest = workspace.payload_digest().unwrap();
    let original_freshness = member.freshness;
    let reboot = format!("CONDUIT_REBOOT_BOOTSEL@1:{BUILD_ID}").into_bytes();
    assert_eq!(reboot.len(), 89);
    assert!(reboot.len() > 64);
    let mut maintenance = Link::new(Mode::ValidAttribution, 1);
    maintenance.control = Some([b"CONDUIT_BOOTSEL_QUERY@1".to_vec(), reboot].into());
    let mut future = Box::pin(preparation::serve_retained_input(
        &mut maintenance,
        &workspace,
    ));
    assert!(std::mem::size_of_val(future.as_ref().get_ref()) < 2048);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let (result, cost) = probe::observe(|| future.as_mut().poll(&mut context));
    assert!(
        result.is_pending(),
        "the actual handler waits after invoking ROM reset"
    );
    drop(future);
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(RESET.with(Cell::get));
    assert_eq!(
        &maintenance.response[..maintenance.response_len],
        b"CONDUIT_REBOOT_BOOTSEL_ACK@1"
    );
    assert_eq!(workspace.payload().unwrap(), prepared.payload);
    assert_eq!(workspace.payload_digest().unwrap(), digest);
    assert_eq!(member.freshness, original_freshness);
    assert!(member.resources.admissions().is_empty());
}

#[test]
fn occupied_input_refuses_another_builds_reboot_without_losing_custody() {
    NOW.with(|n| n.set(100));
    RESET.with(|flag| flag.set(false));
    let mut member = member();
    let mut workspace = HostCommandInput::<LIMIT>::new();
    let mut prepared = Link::new(Mode::Receipt, 1000);
    assert!(run(member.serve_preparation(&mut prepared, &mut workspace)).is_ok());
    let mut maintenance = Link::new(Mode::ValidAttribution, 1);
    maintenance.control =
        Some([format!("CONDUIT_REBOOT_BOOTSEL@1:{}", "f".repeat(64)).into_bytes()].into());
    let (result, cost) = probe::observe(|| {
        run(preparation::serve_retained_input(
            &mut maintenance,
            &workspace,
        ))
    });
    assert!(result.is_ok());
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(!RESET.with(Cell::get));
    assert_eq!(workspace.payload().unwrap(), prepared.payload);
    assert!(
        std::str::from_utf8(&maintenance.response[..maintenance.response_len])
            .unwrap()
            .contains("input-retained")
    );
}

#[path = "command_ingress/expiry.rs"]
mod expiry;
