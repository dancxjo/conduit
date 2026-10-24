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
#[path = "native_face_scope.rs"]
mod scope;
const CONTROL_RESERVE: usize = 8192;
const PREPARATION_BYTES: usize = 4608;
static SLOT: StaticCell<Box<LocalPublicationFace>> = StaticCell::new();
/// Actual task publication custody, bound to the same non-Send executor owner.
pub struct Publisher {
    inner: custody::Publisher<'static>,
}
/// Root retains this private, non-Clone reader; claimed IDs cannot recover it.
pub struct Reader {
    inner: custody::Reader<'static>,
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
    let identity = crate::native_software::source_identity();
    let (_, cadence) = crate::native_software::source_facts();
    for text in [
        identity.source_document_id.as_str(),
        identity.checked_plot_id.as_str(),
        identity.expanded_plot_id.as_str(),
        reservation.body.as_str(),
        reservation.wake.as_str(),
        reservation.active.plan_id.as_str(),
        reservation.active.active_play_id.as_str(),
    ] {
        if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("face-scope-bound");
        }
    }
    let basis = FaceBasis {
        body_id: Some(scope::identity(&reservation.body)?),
        wake_id: Some(scope::identity(&reservation.wake)?),
        source_document_id: Some(identity.source_document_id),
        checked_plot_id: Some(identity.checked_plot_id),
        expanded_plot_id: Some(identity.expanded_plot_id),
        plan_id: Some(reservation.active.plan_id.clone()),
        active_play_id: Some(reservation.active.active_play_id.clone()),
        sign_ids: alloc::vec![],
    };
    let face = RetainedPublicationFace::prepare(
        basis,
        cadence.count.try_into().map_err(|_| "face-count")?,
        cadence.period_ms.checked_mul(2).ok_or("face-clock-bound")?,
    )
    .map_err(|_| "face-preparation")?;
    let face = Box::new(LocalPublicationFace::new(face));
    let stats = esp_alloc::HEAP.stats();
    if stats.size.saturating_sub(stats.current_usage) < CONTROL_RESERVE {
        return Err("face-control-capacity");
    }
    let owner: &'static LocalPublicationFace =
        &**SLOT.try_init(face).ok_or("face-already-claimed")?;
    let (publisher, reader) = owner.claim().map_err(|e| e.code())?;
    Ok((Publisher { inner: publisher }, Reader { inner: reader }))
}
impl Publisher {
    #[inline(never)]
    pub fn publish(&mut self, snapshot: &PublicationSnapshot<'_>, committed_ms: u64) {
        let _ = self.inner.publish(
            PublicationSnapshot {
                channel: snapshot.channel,
                sequence: snapshot.sequence,
                canonical: snapshot.canonical,
            },
            committed_ms,
        );
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
impl Reader {
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
