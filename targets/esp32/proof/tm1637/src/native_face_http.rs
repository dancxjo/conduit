//! Root's read-only HTTP Mask material for the admitted native Face scope.
//! Neither this presentation mailbox nor its public callback grants membership,
//! Source access, radio control or permission to install another renderer.
use conduit_environment_http::{
    show_mailbox::{HttpShowControl, HttpShowMailbox, HttpShowPublisher},
    HttpShowTime, PreparedHttpShow,
};
use conduit_face::{Face, FacePropertyValue};
static SHOW: HttpShowMailbox = HttpShowMailbox::new();
pub type Control = HttpShowControl<'static>;
pub struct Publisher {
    publisher: HttpShowPublisher<'static>,
    lifetime_ms: u64,
}
pub fn prepare(count: u32, stale_after_ms: u64) -> Result<(Publisher, Control), &'static str> {
    let lifetime_ms = stale_after_ms
        .checked_add(1)
        .ok_or("http-mask-clock-bound")?;
    let (publisher, control) = SHOW
        .claim(count, lifetime_ms)
        .map_err(|_| "http-mask-custody")?;
    Ok((
        Publisher {
            publisher,
            lifetime_ms,
        },
        control,
    ))
}
impl Publisher {
    pub fn withdraw(&self) {
        self.publisher.withdraw();
    }
    /// Called under the actual synchronous reader borrow after Face publication.
    /// Fresh material cannot outlive the Face's observed-time freshness window.
    /// Already-stale material gets only an availability lease; no flag is changed.
    pub fn publish(&mut self, face: &Face, now_ms: u64) -> Result<(), &'static str> {
        let show = PreparedHttpShow::prepare(face).map_err(|_| "http-mask-face")?;
        let fresh = face.properties.iter().any(|p| {
            p.subject == "environment"
                && p.name == "fresh"
                && p.value == FacePropertyValue::Flag(true)
        });
        let origin = if fresh {
            match show.time_coordinate() {
                HttpShowTime::ObservedAtBootMs(value) => value,
                _ => return Err("http-mask-observed-clock"),
            }
        } else {
            now_ms
        };
        let until = origin
            .checked_add(self.lifetime_ms)
            .ok_or("http-mask-clock-bound")?;
        if until <= now_ms {
            return Err("http-mask-expired");
        }
        self.publisher
            .publish(show, now_ms, until)
            .map_err(|_| "http-mask-publication")
    }
}
/// Root installs this callback only in the native Face profile's actual AP
/// service. Before publication, after expiry or withdrawal it returns no data.
/// Response material is owned before socket I/O awaits. No Face borrow escapes.
pub fn response(now_ms: u64, json: bool) -> Option<heapless::String<2048>> {
    SHOW.response(true, now_ms, json).ok().flatten()
}
