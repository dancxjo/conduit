//! Actual native continuous worker with deterministic CPU time and admitted
//! test headroom. No ESP32 memory admission, timing, or physical display proof.
extern crate alloc;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[path = "support/mask.rs"]
mod fixture;
#[allow(dead_code)] // Shared fake-pin helpers include separate binder scenarios.
#[path = "support/native_mask.rs"]
mod native;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core as c;
use conduit_environment_runtime::mask_renderer::*;
use conduit_face as f;
use conduit_kernel::{HostCallId, RequestId};
use std::rc::Rc;
fn planned(ready: &ReadyRenderer<native::Provider>) -> f::PlannedMaskPlot {
    let source = conduit_environment_make::SevenSegmentMaskSource::compile().unwrap();
    let mut host = fixture::host();
    host.capabilities
        .retain(|o| o.kind_id.as_str() != f::RENDERER_KIND);
    host.resources.clear();
    ready.append_to_advertisement(&mut host).unwrap();
    source.plan(&host, 8192, &[native::grant(ready)]).unwrap()
}
fn encode(part: f::MaskJsonPart<'_>, out: &mut [u8]) -> Result<usize, f::MaskJsonRefusal> {
    serde_json_core::to_slice(&part, out).map_err(|_| f::MaskJsonRefusal::Capacity)
}

#[path = "native_mask_renderer/leases.rs"]
mod lease_tests;
mod electrical {
    pub type DisplayPins<'a> = super::native::Provider;
}
mod native_authority {
    pub(crate) use crate::lease_tests::production::MaskLeaseIssuer;
}
mod native_display {
    #[allow(unused_imports)] // Lease aliases are also used by target-cfg fixture builds.
    pub use crate::worker::{EvidenceSink, Lease, LeaseSource};
    pub mod task {
        pub use crate::mask_session::RetiredMask;
    }
}
#[allow(dead_code)] // Actual owner API also serves separately tested admission paths.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_mask_session.rs"]
mod mask_session;
use mask_session as session;
#[allow(dead_code)] // Public status/canonical replay are tested independently.
#[path = "../../../../targets/esp32/proof/tm1637/src/native_mask_evidence.rs"]
mod native_mask_evidence;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_mask_worker.rs"]
mod worker;
mod native_face {
    pub struct ReadLease(conduit_environment_runtime::publication_face_custody::ReadLease<'static>);
    impl From<conduit_environment_runtime::publication_face_custody::ReadLease<'static>> for ReadLease {
        fn from(
            value: conduit_environment_runtime::publication_face_custody::ReadLease<'static>,
        ) -> Self {
            Self(value)
        }
    }
    impl ReadLease {
        pub fn with_retained<R>(
            &self,
            consume: impl FnOnce(
                &conduit_face::Face,
                conduit_environment_runtime::publication_face_custody::ReadStatus,
            ) -> R,
        ) -> Result<R, conduit_environment_runtime::publication_face_custody::CustodyRefusal>
        {
            self.0.with_retained(consume)
        }
        pub fn with_current<R>(
            &self,
            now: u64,
            consume: impl FnOnce(
                &conduit_face::Face,
                conduit_environment_runtime::publication_face_custody::ReadStatus,
            ) -> R,
        ) -> Result<R, &'static str> {
            self.0
                .with_current(now, consume)
                .map_err(|error| error.code())
        }
    }
}
// Only the time/headroom/electrical adapters below are synthetic. All checked
// Body/Plan, lease, kernel, manifestation, evidence and worker code is actual.
extern crate self as embassy_time;
extern crate self as esp_alloc;
extern crate self as esp_println;
pub use std::println;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
static CLOCK: AtomicU64 = AtomicU64::new(1000);
static FREE: AtomicUsize = AtomicUsize::new(1_000_000);
thread_local! { static FAIL_PINS: std::cell::RefCell<Option<Rc<native::Pins>>> = const { std::cell::RefCell::new(None) }; }
pub struct Heap;
pub static HEAP: Heap = Heap;
pub struct Stats {
    pub size: usize,
    pub current_usage: usize,
}
impl Heap {
    pub fn free(&self) -> usize {
        FREE.load(Ordering::Relaxed)
    }
    pub fn stats(&self) -> Stats {
        Stats {
            size: self.free(),
            current_usage: 0,
        }
    }
}
pub struct Instant(u64);
impl Instant {
    pub fn now() -> Self {
        Self(CLOCK.load(Ordering::Relaxed))
    }
    pub fn as_millis(&self) -> u64 {
        self.0
    }
}
pub struct Timer;
impl Timer {
    pub async fn after_millis(ms: u64) {
        let now = CLOCK.fetch_add(ms, Ordering::Relaxed) + ms;
        if now >= 4000 {
            FAIL_PINS.with(|slot| {
                if let Some(pins) = slot.borrow().as_ref() {
                    pins.nack.set(true);
                }
            });
        }
    }
}
struct Limited(lease_tests::production::MaskLeaseIssuer);
impl worker::LeaseSource for Limited {
    // A finite narrower owner scope, while the actual Root scope stays retained.
    fn current(&self, now: u64) -> bool {
        now < 10000
    }
    fn next(
        &mut self,
        owner: &mask_session::NativeReady,
        now: u64,
    ) -> Result<Option<worker::Lease>, &'static str> {
        self.0.next(owner, now).map(Some)
    }
    fn stage_successor(
        &mut self,
        native: &RendererIdentity,
        prior: &c::ActivePlayIdentity,
        now: u64,
    ) -> Result<Option<worker::Lease>, &'static str> {
        self.0.stage_successor(native, prior, now)
    }
    fn replaced(
        &mut self,
        native: &RendererIdentity,
        prior: &c::ActivePlayIdentity,
        old: &c::CapabilityInspection,
        new: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.0.validate_replaced(native, prior, old, new)
    }
    fn retired(
        &mut self,
        owner: &mask_session::NativeReady,
        active: &c::ActivePlayIdentity,
        inspection: &c::CapabilityInspection,
    ) -> Result<(), &'static str> {
        self.0.validate_retired(owner, active, inspection)
    }
}
struct Evidence {
    consumer: native_mask_evidence::Consumer,
    kinds: Vec<&'static str>,
    refuse_once: bool,
}
impl Evidence {
    fn new(refuse_once: bool) -> Self {
        Self {
            consumer: native_mask_evidence::Consumer::new(),
            kinds: vec![],
            refuse_once,
        }
    }
}
impl worker::EvidenceSink for Evidence {
    fn accept(
        &mut self,
        n: u64,
        active: &c::ActivePlayIdentity,
        bytes: &[u8],
        signs: &mask_session::Log,
    ) -> Result<(), &'static str> {
        self.consumer.accept(n, active, bytes, signs)?;
        self.kinds.push("closed");
        Ok(())
    }
    fn accept_replaced(
        &mut self,
        n: u64,
        active: &c::ActivePlayIdentity,
        bytes: &[u8],
        signs: &mask_session::Log,
    ) -> Result<(), &'static str> {
        if self.refuse_once {
            self.refuse_once = false;
            return Err("fixture-evidence-refused");
        }
        self.consumer.accept_replaced(n, active, bytes, signs)?;
        self.kinds.push("replaced");
        Ok(())
    }
}
struct Fixture {
    session: Box<mask_session::NativeMaskSession>,
    leases: Limited,
    evidence: Evidence,
    pins: Rc<native::Pins>,
    _publisher: conduit_environment_runtime::publication_face_custody::Publisher<'static>,
    _reader: conduit_environment_runtime::publication_face_custody::Reader<'static>,
    _guard: lease_tests::production::MaskScopeGuard,
    _body: conduit_body::SealedBodyLifecycleSession,
}
fn fixture(refuse_once: bool) -> Fixture {
    use conduit_body::BodyPlanEvidence;
    use conduit_environment_runtime::mask_kernel::PreparedMaskKernel;
    use lease_tests::{new_body, prepare_at, start_at};
    use mask_session as ms;
    let sequence_base = 0;
    let values = Box::leak(Box::new(
        conduit_kernel::FixedValueStore::<2, 8192>::new(16384).unwrap(),
    ));
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let (root, leases, capsule, guard) = prepare_at(&ready, &planned, sequence_base);
    let mut body = new_body(&planned);
    start_at(&mut body, &root.host, &root.boot, sequence_base + 1);
    let mut metadata = planned.plan.clone();
    let mut configs = std::collections::BTreeMap::new();
    for gear in &mut metadata.fragments[0].placements {
        configs.insert(
            gear.placement_id.clone(),
            core::mem::take(&mut gear.configuration),
        );
    }
    let verified = c::verify_plan_metadata_with_configurations(metadata, |g| {
        Ok::<_, ()>(configs[&g.placement_id].clone())
    })
    .unwrap();
    let current = body.realization().unwrap();
    let routes = f::AdmittedMaskPlotRoutes::from_sealed_session(
        &body,
        &planned.mask,
        &verified,
        vec![f::SealedMaskPlotRoute {
            route_id: "route/completed/native".into(),
            mask_plot: planned.mask.plot_identity.clone(),
            plan_id: current.plan.plan_id().clone(),
            child_mask_plan_id: None,
            owner_route_seal_id: None,
            placement_ids: verified.preparation_metadata().fragments[0]
                .placements
                .iter()
                .map(|g| g.placement_id.clone())
                .collect(),
            currently_available: true,
        }],
    )
    .unwrap();
    let wardrobe = f::MaskWardrobeControl::new_from_admitted_routes(
        f::BodyMaskWardrobe::new(
            routes.body_id().clone(),
            Some(current.plan.wake_id().clone()),
            f::MaskWardrobe::new(
                f::MaskWardrobeLifetime::Wake,
                vec![planned.mask.plot_identity.clone()],
                vec![planned.mask.plot_identity.clone()],
            )
            .unwrap(),
        )
        .unwrap(),
        &routes,
        None,
    )
    .unwrap();
    let initial = c::bind_active_play(
        &planned.plan.plan_id,
        &root.host,
        &root.boot,
        sequence_base + 1,
    );
    let context = f::PreparedMaskShowContext::prepare(&planned, 8192).unwrap();
    let kernel = PreparedMaskKernel::prepare_parts(&planned)
        .unwrap()
        .finish_with_fixed_storage(values)
        .unwrap();
    let face_owner = Box::leak(Box::new(
        conduit_environment_runtime::publication_face_custody::LocalPublicationFace::new(
            conduit_environment_runtime::retained_publication_face::RetainedPublicationFace::prepare(
                f::FaceBasis {
 body_id: Some(routes.body_id().clone()), wake_id: Some(current.wake.wake_id.clone()),
 source_document_id: Some(planned.mask.plot_identity.source_document_id.clone()),
 checked_plot_id: Some(planned.mask.plot_identity.checked_plot_id.clone()),
 expanded_plot_id: Some(planned.mask.plot_identity.expanded_plot_id.clone()),
 plan_id: Some(initial.plan_id.clone()), active_play_id: Some(initial.active_play_id.clone()), sign_ids: vec![],
}, 960, 120000).unwrap())));
    let (mut face_publisher, face_reader) = face_owner.claim().unwrap();
    let reader = face_reader.lease().into();
    let mut owner = Some(ready);
    let input = ms::Preparation {
        context,
        kernel,
        invocation: capsule.into_execution(),
        routes,
        wardrobe,
        producer: ms::FaceScope {
            producer: planned.mask.plot_identity.clone(),
            active: initial.clone(),
            wake: current.wake.wake_id.clone(),
        },
        reader,
    };
    let session = ms::NativeMaskSession::prepare_retained(&mut owner, input)
        .unwrap_or_else(|_| panic!("checked actual session preparation refused"));
    let mut leases = Box::new(leases);

    assert!(session.fresh_for_task());
    assert!(leases.fresh_for_task());
    leases.activate(&body, 1000).unwrap();

    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let mut decoder =
        conduit_environment_runtime::observation::PreparedSourceFrameDecoder::prepare(&contract)
            .unwrap();
    let reading = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
    face_publisher
        .publish(
            conduit_environment_runtime::publication::PublicationSnapshot {
                channel: conduit_environment_runtime::publication::PublicationChannel::Reading,
                sequence: 1,
                canonical: reading,
            },
            1000,
        )
        .unwrap();
    Fixture {
        session,
        leases: Limited(*leases),
        evidence: Evidence::new(refuse_once),
        pins,
        _publisher: face_publisher,
        _reader: face_reader,
        _guard: guard,
        _body: body,
    }
}
fn run(fixture: &mut Fixture) -> Result<u64, &'static str> {
    let mut future = std::pin::pin!(fixture
        .session
        .run(&mut fixture.leases, &mut fixture.evidence));
    let waker = std::task::Waker::noop();
    let mut context = std::task::Context::from_waker(waker);
    match std::future::Future::poll(future.as_mut(), &mut context) {
        std::task::Poll::Ready(result) => result,
        std::task::Poll::Pending => panic!("deterministic clock adapter unexpectedly pending"),
    }
}
#[test]
fn actual_worker_continuity_refusal_cleanup_and_terminal_evidence() {
    CLOCK.store(1000, Ordering::Relaxed);
    let mut good = fixture(false);
    assert_eq!(run(&mut good), Ok(3));
    assert_eq!(good.evidence.kinds, ["replaced", "replaced", "closed"]);
    assert!(good.session.call.is_none());
    assert!(good.session.ready.is_some());
    assert!(good
        .session
        .handoff
        .as_ref()
        .unwrap()
        .previous_log
        .is_none());
    assert!(good.session.handoff.as_ref().unwrap().receipt.is_none());

    CLOCK.store(1000, Ordering::Relaxed);
    let mut interrupted = fixture(false);
    FAIL_PINS.with(|slot| *slot.borrow_mut() = Some(interrupted.pins.clone()));
    assert_eq!(run(&mut interrupted), Err("mask-present-wire-nack"));
    FAIL_PINS.with(|slot| *slot.borrow_mut() = None);
    assert!(interrupted.evidence.kinds.is_empty());
    assert!(interrupted.session.ready.is_none());
    assert!(interrupted.session.call.is_some());
    assert!(interrupted
        .session
        .handoff
        .as_ref()
        .unwrap()
        .successor
        .is_some());
    assert!(interrupted
        .session
        .handoff
        .as_ref()
        .unwrap()
        .previous_log
        .is_some());
    assert!(interrupted
        .session
        .handoff
        .as_ref()
        .unwrap()
        .receipt
        .is_none());
    assert_eq!(
        interrupted.session.call.as_ref().unwrap().state(),
        RendererState::Failed
    );
    assert_eq!(
        interrupted.session.cleanup_refusal(),
        Some("mask-withdraw-cleanup")
    );

    CLOCK.store(1000, Ordering::Relaxed);
    let mut refused = fixture(true);
    assert_eq!(run(&mut refused), Err("fixture-evidence-refused"));
    assert!(refused.session.pending_evidence());
    assert!(refused
        .session
        .handoff
        .as_ref()
        .unwrap()
        .previous_log
        .is_some());
    assert!(refused.session.handoff.as_ref().unwrap().receipt.is_some());
    assert_eq!(
        refused.session.call.as_ref().unwrap().state(),
        RendererState::Withdrawn
    );
    refused
        .session
        .accept_pending(&mut refused.leases, &mut refused.evidence)
        .unwrap();
    assert_eq!(refused.evidence.kinds, ["replaced"]);
    assert!(!refused.session.pending_evidence());
    assert!(refused
        .session
        .handoff
        .as_ref()
        .unwrap()
        .previous_log
        .is_none());
    assert!(refused.session.handoff.as_ref().unwrap().receipt.is_none());
    assert!(
        refused.session.faulted,
        "old evidence acceptance cannot revive withdrawn successor"
    );
    assert!(refused.session.ready.is_none());
    assert!(refused.session.call.is_some());

    CLOCK.store(1000, Ordering::Relaxed);
    FREE.store(16384, Ordering::Relaxed);
    let mut capacity = fixture(false);
    assert_eq!(run(&mut capacity), Ok(3));
    assert!(capacity.session.handoff.is_none());
    assert_eq!(capacity.evidence.kinds, ["closed", "closed", "closed"]);
    FREE.store(1_000_000, Ordering::Relaxed);
}
