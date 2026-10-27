//! One pre-admitted producer Face owned by the actual native task. It grants no
//! Body membership, Mask route, Line, GPIO or storage authority.
use crate::native_owner_reservation::OwnerReservation;
use alloc::boxed::Box;
use conduit_environment_runtime::{
    publication::PublicationSnapshot,
    publication_face_custody::{self as custody, LocalPublicationFace},
    retained_publication_face::RetainedPublicationFace,
};
use conduit_face::{Face, FaceBasis, FacePropertyValue};
use static_cell::StaticCell;
#[path = "native_face_http.rs"]
mod http;
#[path = "native_face_reunion.rs"]
mod reunion;
pub use reunion::{Replacing, Returned, Returning};
/// No replacement Play or history outbox is admitted by returning Face custody.
#[allow(clippy::type_complexity)]
pub fn reunite(
    publisher: Publisher,
    reader: Reader,
) -> Result<Returned, (Returning, &'static str)> {
    reunion::reunite(publisher, reader)
}
/// Independently prepared scope. Fields cannot be reconstructed from IDs alone.
pub struct PreparedReplacement {
    face: RetainedPublicationFace,
    count: u32,
    stale_after_ms: u64,
    active: [u8; 64],
}
#[cfg(feature = "body-image-root")]
#[path = "native_local_face_scope.rs"]
mod local_scope;
#[path = "native_face_scope.rs"]
mod scope;
pub fn http_response(
    now_ms: u64,
    json: bool,
    after: Option<u32>,
) -> Option<heapless::String<2048>> {
    if let Some(after) = after {
        #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
        return crate::native_observed_http::response(after);
        #[cfg(not(any(feature = "local-evidence-store", feature = "observation-store")))]
        {
            let _ = after;
            return None;
        }
    }
    http::response(now_ms, json)
}
const CONTROL_RESERVE: usize = 8192;
const PREPARATION_BYTES: usize = 4608;
static SLOT: StaticCell<Box<LocalPublicationFace>> = StaticCell::new();
/// Actual task publication custody, bound to the same non-Send executor owner.
pub struct Publisher {
    inner: custody::Publisher<'static>,
    reader: ReadLease,
    http: http::Publisher,
}
/// Root retains this private, non-Clone reader; claimed IDs cannot recover it.
pub struct Reader {
    inner: custody::Reader<'static>,
    http: http::Control,
}
#[derive(Clone, Copy, serde::Serialize)]
pub struct Inspection {
    pub source_sequence: u64,
    pub revision: u64,
    pub content_digest: [u8; 32],
    pub publisher_retained: bool,
    pub fresh: bool,
    pub available: bool,
    pub refusal: Option<&'static str>,
}
/// Root calls this only inside its exact reserved native preparation, before
/// task registration. Owner backing, dynamic data and control headroom are
/// admitted here, without polling a Step.
#[inline(never)]
pub fn prepare(reservation: &OwnerReservation) -> Result<(Publisher, Reader), &'static str> {
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < PREPARATION_BYTES + CONTROL_RESERVE {
        return Err("face-preparation-capacity");
    }
    let body: conduit_body::BodyId = scope::identity(&reservation.body)?;
    let wake: conduit_body::WakeId = scope::identity(&reservation.wake)?;
    #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
    crate::native_observed_archive::bind(reservation)?;
    prepare_scope(&body, &wake, &reservation.active)
}
/// Prepare publication custody for the exact installed local proposal. This
/// does not start the Body or authorize polling the producer's kernel.
#[cfg(feature = "body-image-root")]
pub fn prepare_local(
    body: &conduit_body::SealedBodyLifecycleSession,
    plan: &conduit_core::VerifiedPlanMetadata,
    fragment: &conduit_core::FragmentId,
    active: &conduit_core::ActivePlayIdentity,
) -> Result<(Publisher, Reader), &'static str> {
    let source = crate::native_software::source_identity();
    let metadata = plan.preparation_metadata();
    if metadata.source_document_id != source.source_document_id
        || metadata.checked_plot_id != source.checked_plot_id
        || metadata.expanded_plot_id != source.expanded_plot_id
    {
        return Err("local-face-source");
    }
    let (body_id, wake_id) = local_scope::check(body, plan, fragment, active)?;
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < PREPARATION_BYTES + CONTROL_RESERVE {
        return Err("face-preparation-capacity");
    }
    #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
    crate::native_observed_archive::bind_local(body_id.as_str(), active)?;
    prepare_scope(body_id, wake_id, active)
}
/// Prepare from the current opaque Body seal after checked Plot metadata was
/// retired. No received identity replaces that seal or the linked Source.
#[cfg(feature = "body-image-root")]
pub fn prepare_installed_local(
    body: &conduit_body::SealedBodyLifecycleSession,
    fragment: &conduit_core::FragmentId,
    active: &conduit_core::ActivePlayIdentity,
    generation: conduit_core::OfferGeneration,
) -> Result<(Publisher, Reader), &'static str> {
    let source = crate::native_software::source_identity();
    let (body_id, wake_id) =
        local_scope::check_installed(body, &source, fragment, active, generation)?;
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < PREPARATION_BYTES + CONTROL_RESERVE {
        return Err("face-preparation-capacity");
    }
    #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
    crate::native_observed_archive::bind_local(body_id.as_str(), active)?;
    prepare_scope(body_id, wake_id, active)
}
/// Prepare a fresh Face from the current canonical installed proposal. This
/// does not bind a history outbox, start workers, or grant a replacement Play.
#[cfg(feature = "body-image-root")]
pub fn prepare_installed_replacement(
    body: &conduit_body::SealedBodyLifecycleSession,
    fragment: &conduit_core::FragmentId,
    active: &conduit_core::ActivePlayIdentity,
    generation: conduit_core::OfferGeneration,
) -> Result<PreparedReplacement, &'static str> {
    let source = crate::native_software::source_identity();
    let (body, wake) = local_scope::check_installed(body, &source, fragment, active, generation)?;
    prepare_replacement_scope(body, wake, active)
}
fn prepare_scope(
    body: &conduit_body::BodyId,
    wake: &conduit_body::WakeId,
    active: &conduit_core::ActivePlayIdentity,
) -> Result<(Publisher, Reader), &'static str> {
    let PreparedReplacement {
        face,
        count,
        stale_after_ms,
        active: _,
    } = prepare_replacement_scope(body, wake, active)?;
    let face = Box::new(LocalPublicationFace::new(face));
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < CONTROL_RESERVE {
        return Err("face-control-capacity");
    }
    let owner: &'static LocalPublicationFace =
        &**SLOT.try_init(face).ok_or("face-already-claimed")?;
    let (publisher, reader) = owner.claim().map_err(|e| e.code())?;
    let (http, control) = http::prepare(count, stale_after_ms)?;
    Ok(pair(publisher, reader, http, control))
}
fn pair(
    inner: custody::Publisher<'static>,
    inner_reader: custody::Reader<'static>,
    http: http::Publisher,
    control: http::Control,
) -> (Publisher, Reader) {
    let reader = Reader {
        inner: inner_reader,
        http: control,
    };
    let lease = reader.lease();
    (
        Publisher {
            inner,
            reader: lease,
            http,
        },
        reader,
    )
}
fn prepare_replacement_scope(
    body: &conduit_body::BodyId,
    wake: &conduit_body::WakeId,
    active: &conduit_core::ActivePlayIdentity,
) -> Result<PreparedReplacement, &'static str> {
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < PREPARATION_BYTES + CONTROL_RESERVE {
        return Err("face-preparation-capacity");
    }
    let identity = crate::native_software::source_identity();
    let (_, cadence) = crate::native_software::source_facts();
    for text in [
        identity.source_document_id.as_str(),
        identity.checked_plot_id.as_str(),
        identity.expanded_plot_id.as_str(),
        body.as_str(),
        wake.as_str(),
        active.plan_id.as_str(),
        active.active_play_id.as_str(),
    ] {
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("face-scope-bound");
        }
    }
    let basis = FaceBasis {
        body_id: Some(body.clone()),
        wake_id: Some(wake.clone()),
        source_document_id: Some(identity.source_document_id),
        checked_plot_id: Some(identity.checked_plot_id),
        expanded_plot_id: Some(identity.expanded_plot_id),
        plan_id: Some(active.plan_id.clone()),
        active_play_id: Some(active.active_play_id.clone()),
        sign_ids: alloc::vec![],
    };
    let count = cadence.count.try_into().map_err(|_| "face-count")?;
    let stale_after_ms = cadence.period_ms.checked_mul(2).ok_or("face-clock-bound")?;
    let face = RetainedPublicationFace::prepare(basis, count, stale_after_ms)
        .map_err(|_| "face-preparation")?;
    stale_after_ms
        .checked_add(1)
        .ok_or("http-mask-clock-bound")?;
    let mut active_id = [0; 64];
    active_id.copy_from_slice(active.active_play_id.as_str().as_bytes());
    Ok(PreparedReplacement {
        face,
        count,
        stale_after_ms,
        active: active_id,
    })
}

fn read_scope(reader: &ReadLease) -> Result<[u8; 64], &'static str> {
    reader
        .inner
        .with_retained(|face, _| {
            let value = face
                .basis
                .active_play_id
                .as_ref()
                .ok_or("face-return-scope")?;
            let bytes = value.as_str().as_bytes();
            if bytes.len() != 64 {
                return Err("face-return-scope");
            }
            let mut active = [0; 64];
            active.copy_from_slice(bytes);
            Ok(active)
        })
        .map_err(|e| e.code())?
}
impl Publisher {
    /// Close presentation material while retaining actual publication custody.
    pub(crate) fn withdraw_http(&self) {
        self.http.withdraw();
    }
    #[inline(never)]
    pub fn publish(&mut self, snapshot: &PublicationSnapshot<'_>, committed_ms: u64) {
        let published = self.inner.publish(
            PublicationSnapshot {
                channel: snapshot.channel,
                sequence: snapshot.sequence,
                canonical: snapshot.canonical,
            },
            committed_ms,
        );
        if published.is_err() {
            self.http.withdraw();
            return;
        }
        #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
        let _ = read_scope(&self.reader).and_then(|active| {
            use sha2::{Digest, Sha256};
            // The exact-generation lease supplied this checked Face basis.
            // Its borrow ends before the archive signer or flash is entered.
            let play: [u8; 32] = Sha256::digest(active).into();
            crate::native_observed_archive::append(
                PublicationSnapshot {
                    channel: snapshot.channel,
                    sequence: snapshot.sequence,
                    canonical: snapshot.canonical,
                },
                committed_ms,
                &play,
            )
        });
        self.publish_http(committed_ms);
    }
    // Keep Mask preparation scratch out of the deeper quantity validation call
    // chain used by inner.publish. These phases never need their scratch live
    // together, even though both belong to one committed publication.
    #[inline(never)]
    fn publish_http(&mut self, committed_ms: u64) {
        let prepared = self.reader.with_current(committed_ms, |face, status| {
            if !status.publisher_retained || status.refusal.is_some() {
                return Err("http-mask-reader-unavailable");
            }
            self.http.publish(face, committed_ms)
        });
        if !matches!(prepared, Ok(Ok(()))) {
            self.http.withdraw();
        }
    }
}
fn digest(face: &Face) -> [u8; 32] {
    let mut result = [0; 32];
    for (out, pair) in result
        .iter_mut()
        .zip(face.identity.as_str().as_bytes().chunks_exact(2))
    {
        let hex = |b| {
            b"0123456789abcdef"
                .iter()
                .position(|c| *c == b)
                .expect("sealed Face digest") as u8
        };
        *out = (hex(pair[0]) << 4) | hex(pair[1]);
    }
    result
}
/// Copyable exact-generation local read capability, without return authority.
#[derive(Clone, Copy)]
pub struct ReadLease {
    inner: custody::ReadLease<'static>,
}
impl Reader {
    pub fn lease(&self) -> ReadLease {
        ReadLease {
            inner: self.inner.lease(),
        }
    }
    pub fn inspect(&self, now: u64) -> Result<Inspection, &'static str> {
        self.lease().inspect(now)
    }
    pub fn withdraw_http(&self) {
        self.http.withdraw();
    }
}
impl ReadLease {
    /// Synchronous local access only; Root still admits the consuming Mask and
    /// its exact route/work. The borrowed Face cannot escape this callback.
    pub fn with_current<R>(
        &self,
        now: u64,
        consume: impl FnOnce(&Face, custody::ReadStatus) -> R,
    ) -> Result<R, &'static str> {
        self.inner.with_current(now, consume).map_err(|e| e.code())
    }
    pub fn inspect(&self, now: u64) -> Result<Inspection, &'static str> {
        self.with_current(now, |value, status| {
            let property = |name| value.properties.iter().find(|p| p.name == name).map(|p| &p.value);
            Inspection {
                source_sequence: status.source_sequence,
                revision: value.revision, content_digest: digest(value),
                publisher_retained: status.publisher_retained,
                fresh: matches!(property("fresh"), Some(FacePropertyValue::Flag(true))),
                available: status.refusal.is_none() && matches!(property("sensor-status"), Some(FacePropertyValue::Text(t)) if t == "available"),
                refusal: status.refusal,
            }
        })
    }
}
