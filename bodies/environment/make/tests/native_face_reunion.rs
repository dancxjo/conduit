//! Actual composite transition and actual Face/HTTP custody. The native outer
//! handle adapter injects Busy at the HTTP boundary; real mailbox Busy is
//! independently covered by its crate unit proof. No ESP executor claim.
extern crate alloc;
use conduit_environment_runtime::{
    publication_face_custody as custody, retained_publication_face::RetainedPublicationFace,
};
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_http.rs"]
#[allow(dead_code)]
mod actual_http;
mod http {
    pub use super::actual_http::{Control, Publisher};
    use core::sync::atomic::{AtomicBool, Ordering};
    pub static BUSY_RETURN: AtomicBool = AtomicBool::new(false);
    pub static BUSY_REPLACE: AtomicBool = AtomicBool::new(false);
    pub struct Returned(super::actual_http::Returned);
    pub fn reunite(
        p: Publisher,
        c: Control,
    ) -> Result<Returned, (Publisher, Control, &'static str)> {
        if BUSY_RETURN.swap(false, Ordering::SeqCst) {
            return Err((p, c, "injected-http-busy"));
        }
        super::actual_http::reunite(p, c).map(Returned)
    }
    impl Returned {
        pub fn replace(
            self,
            count: u32,
            stale: u64,
        ) -> Result<(Publisher, Control), (Self, &'static str)> {
            if BUSY_REPLACE.swap(false, Ordering::SeqCst) {
                return Err((self, "injected-http-busy"));
            }
            self.0
                .replace(count, stale)
                .map_err(|(returned, e)| (Self(returned), e))
        }
    }
}
pub struct Publisher {
    inner: custody::Publisher<'static>,
    reader: custody::ReadLease<'static>,
    http: http::Publisher,
}
pub struct Reader {
    inner: custody::Reader<'static>,
    http: http::Control,
}
pub struct PreparedReplacement {
    face: RetainedPublicationFace,
    count: u32,
    stale_after_ms: u64,
    active: [u8; 64],
}
fn pair(
    inner: custody::Publisher<'static>,
    reader: custody::Reader<'static>,
    http: http::Publisher,
    control: http::Control,
) -> (Publisher, Reader) {
    let lease = reader.lease();
    (
        Publisher {
            inner,
            reader: lease,
            http,
        },
        Reader {
            inner: reader,
            http: control,
        },
    )
}
#[path = "../../../../targets/esp32/proof/tm1637/src/native_face_reunion.rs"]
mod transaction;
fn scope(label: &str) -> [u8; 64] {
    use sha2::Digest;
    let text = format!("{:x}", sha2::Sha256::digest(label.as_bytes()));
    text.as_bytes().try_into().unwrap()
}
fn read_scope(reader: &custody::ReadLease<'static>) -> Result<[u8; 64], &'static str> {
    reader
        .with_retained(|face, _| {
            face.basis
                .active_play_id
                .as_ref()
                .unwrap()
                .as_str()
                .as_bytes()
                .try_into()
                .unwrap()
        })
        .map_err(|e| e.code())
}
fn face(label: &str) -> RetainedPublicationFace {
    RetainedPublicationFace::prepare(
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/test".into()),
            checked_plot_id: Some("checked/test".into()),
            expanded_plot_id: Some("expanded/test".into()),
            plan_id: Some("plan/test".into()),
            active_play_id: Some(String::from_utf8(scope(label).to_vec()).unwrap().into()),
            sign_ids: vec![],
        },
        3,
        100,
    )
    .unwrap()
}
#[test]
#[allow(clippy::result_large_err)] // The refusal preserves the complete affine owner bundle.
fn actual_composite_transition_keeps_every_partial_owner_until_both_replacements_complete() {
    println!(
        "composite_cpu_bytes returning={} returned={} replacing={} prepared={} pointer_bits={}",
        core::mem::size_of::<transaction::Returning>(),
        core::mem::size_of::<transaction::Returned>(),
        core::mem::size_of::<transaction::Replacing>(),
        core::mem::size_of::<PreparedReplacement>(),
        usize::BITS
    );
    let owner = Box::leak(Box::new(custody::LocalPublicationFace::new(face("old"))));
    let (p, r) = owner.claim().unwrap();
    let old = r.lease();
    let (http, control) = actual_http::prepare(3, 100).unwrap();
    let (p, r) = pair(p, r, http, control);
    // Wrong actual Face ownership refuses without advancing either generation.
    let wrong_owner = Box::leak(Box::new(custody::LocalPublicationFace::new(face("other"))));
    let (wrong_publisher, wrong_reader) = wrong_owner.claim().unwrap();
    let Reader {
        inner: genuine_reader,
        http: control,
    } = r;
    let wrong = Reader {
        inner: wrong_reader,
        http: control,
    };
    let (pending, reason) = transaction::reunite(p, wrong).err().unwrap();
    assert_eq!(reason, "face-custody-scope");
    let (p, wrong) = pending.into_original().ok().unwrap();
    let r = Reader {
        inner: genuine_reader,
        http: wrong.http,
    };
    let _wrong_reader = wrong.inner;
    drop(wrong_publisher);
    assert!(old.with_retained(|_, _| ()).is_ok());
    // Actual active Face borrow refuses before either owner advances.
    let attempt = old
        .with_retained(|_, _| transaction::reunite(p, r))
        .unwrap();
    let (pending, e) = attempt.err().unwrap();
    assert_eq!(e, "face-busy");
    let (p, r) = pending.into_original().ok().unwrap();
    assert!(old.with_retained(|_, _| ()).is_ok());
    http::BUSY_RETURN.store(true, core::sync::atomic::Ordering::SeqCst);
    let ((pending, e), memory) =
        allocation_probe::observe(|| transaction::reunite(p, r).err().unwrap());
    assert_eq!(e, "injected-http-busy");
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(matches!(
        old.with_retained(|_, _| ()),
        Err(custody::CustodyRefusal::Scope)
    ));
    assert!(actual_http::response(0, true).is_none());
    let pending = pending
        .into_original()
        .err()
        .expect("old actual Face pair already consumed");
    let returned = pending.retry().ok().unwrap();
    let same = PreparedReplacement {
        face: face("old"),
        count: 99,
        stale_after_ms: 100,
        active: scope("old"),
    };
    let ((unchanged, reason), memory) =
        allocation_probe::observe(|| returned.replace(same).err().unwrap());
    assert_eq!(reason, "face-replacement-same-play");
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let (returned, same) = unchanged.into_unstarted().ok().unwrap();
    assert_eq!(same.count, 99);
    assert!(actual_http::response(0, true).is_none());
    let replacement = PreparedReplacement {
        face: face("fresh"),
        count: 3,
        stale_after_ms: 100,
        active: scope("fresh"),
    };
    http::BUSY_REPLACE.store(true, core::sync::atomic::Ordering::SeqCst);
    let ((pending, e), memory) =
        allocation_probe::observe(|| returned.replace(replacement).err().unwrap());
    assert_eq!(e, "injected-http-busy");
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(matches!(
        old.with_current(0, |_, _| ()),
        Err(custody::CustodyRefusal::Scope)
    ));
    assert!(actual_http::response(0, true).is_none());
    let pending = pending
        .into_unstarted()
        .err()
        .expect("new Face pair cannot escape while HTTP is still returned");
    let ((mut p, r), memory) = allocation_probe::observe(|| pending.retry().ok().unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert!(p
        .reader
        .with_retained(|f, _| assert_eq!(
            f.basis.active_play_id.as_ref().unwrap().as_str(),
            String::from_utf8(scope("fresh").to_vec()).unwrap()
        ))
        .is_ok());
    let current = p.reader;
    current
        .with_retained(|face, _| p.http.publish(face, 0))
        .unwrap()
        .unwrap();
    assert!(actual_http::response(0, true).is_some());
    let latest = r.inner.lease();
    let returned = transaction::reunite(p, r).ok().unwrap();
    assert!(matches!(
        latest.with_retained(|_, _| ()),
        Err(custody::CustodyRefusal::Scope)
    ));
    let (p, _r) = returned
        .replace(PreparedReplacement {
            face: face("third"),
            count: 1,
            stale_after_ms: 10,
            active: scope("third"),
        })
        .ok()
        .unwrap();
    p.http.withdraw();
}
