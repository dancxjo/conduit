//! Process-local single-executor custody for one prepared producer Face.
//! IDs and these handles confer no Body membership, routing or hardware grant.
use crate::{
    publication::PublicationSnapshot,
    retained_publication_face::{
        RetainedPublicationBacking, RetainedPublicationFace, RetainedPublicationRefusal,
    },
};
use conduit_environment_face::PublicationRefusal;
use conduit_face::{Face, FaceBasis};
enum FaceState {
    Unbound(Option<RetainedPublicationBacking>),
    Bound(RetainedPublicationFace),
}
impl FaceState {
    fn bound(&self) -> Result<&RetainedPublicationFace, CustodyRefusal> {
        match self {
            Self::Bound(face) => Ok(face),
            Self::Unbound(_) => Err(CustodyRefusal::Unbound),
        }
    }
    fn bound_mut(&mut self) -> Result<&mut RetainedPublicationFace, CustodyRefusal> {
        match self {
            Self::Bound(face) => Ok(face),
            Self::Unbound(_) => Err(CustodyRefusal::Unbound),
        }
    }
}
use core::cell::{Cell, RefCell};

pub struct LocalPublicationFace {
    face: RefCell<FaceState>,
    claimed: Cell<bool>,
    retained: Cell<bool>,
    refusal: Cell<Option<&'static str>>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum CustodyRefusal {
    AlreadyClaimed,
    Unbound,
    AlreadyBound,
    Capacity,
    Busy,
    Publication(RetainedPublicationRefusal),
    Clock(PublicationRefusal),
}
impl CustodyRefusal {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::AlreadyClaimed => "face-already-claimed",
            Self::Unbound => "face-unbound",
            Self::AlreadyBound => "face-already-bound",
            Self::Capacity => "face-capacity",
            Self::Busy => "face-busy",
            Self::Publication(reason) => reason.code(),
            Self::Clock(_) => "clock-reversed",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadStatus {
    pub source_sequence: u64,
    pub publisher_retained: bool,
    pub refusal: Option<&'static str>,
}
/// The private borrow is not Send/Sync. Runtime ownership remains on its actual
/// executor; no interrupt mutex is held while decoding or rendering quantities.
/// ```compile_fail
/// use conduit_environment_runtime::publication_face_custody::Publisher;
/// fn require_send<T: Send>() {}
/// require_send::<Publisher<'static>>();
/// ```
pub struct Publisher<'a> {
    owner: &'a LocalPublicationFace,
}
/// One non-Clone reader, not an admitted Mask caller or remote Line.
/// ```compile_fail
/// use conduit_environment_runtime::publication_face_custody::Reader;
/// fn require_send<T: Send>() {}
/// require_send::<Reader<'static>>();
/// ```
pub struct Reader<'a> {
    owner: &'a LocalPublicationFace,
}
impl LocalPublicationFace {
    /// Actual owned payload, excluding this inline owner and allocator overhead.
    /// Root separately charges incoming actual basis storage before binding.
    pub fn retained_backing_bytes(&self) -> Result<usize, CustodyRefusal> {
        let state = self.face.try_borrow().map_err(|_| CustodyRefusal::Busy)?;
        let bytes = match &*state {
            FaceState::Unbound(Some(backing)) => backing.retained_backing_bytes(),
            FaceState::Bound(face) => face.retained_backing_bytes(),
            FaceState::Unbound(None) => return Err(CustodyRefusal::Unbound),
        };
        bytes.ok_or(CustodyRefusal::Capacity)
    }

    pub fn new(face: RetainedPublicationFace) -> Self {
        Self {
            face: RefCell::new(FaceState::Bound(face)),
            claimed: Cell::new(false),
            retained: Cell::new(false),
            refusal: Cell::new(None),
        }
    }
    /// Own real inert storage before a Plan exists. Handles obtained now have
    /// no readable Face, publication, membership or execution authority.
    pub fn from_backing(backing: RetainedPublicationBacking) -> Self {
        Self {
            face: RefCell::new(FaceState::Unbound(Some(backing))),
            claimed: Cell::new(false),
            retained: Cell::new(false),
            refusal: Cell::new(None),
        }
    }
    /// Root supplies actual admitted execution provenance. Invalid scope returns
    /// the original allocation to its inert state; a bound scope cannot be
    /// silently replaced while its original Play/services retain ownership.
    pub fn bind_scope(&self, basis: FaceBasis) -> Result<(), CustodyRefusal> {
        let mut state = self
            .face
            .try_borrow_mut()
            .map_err(|_| CustodyRefusal::Busy)?;
        let FaceState::Unbound(slot) = &mut *state else {
            return Err(CustodyRefusal::AlreadyBound);
        };
        let backing = slot.take().ok_or(CustodyRefusal::Unbound)?;
        match backing.try_bind(basis) {
            Ok(face) => {
                *state = FaceState::Bound(face);
                Ok(())
            }
            Err((backing, reason)) => {
                *slot = Some(backing);
                Err(CustodyRefusal::Publication(reason))
            }
        }
    }
    /// Borrow the actual retained Face for read-only inspection. This does not
    /// claim a producer/renderer handle or grant publication or routing rights.
    pub fn with_retained<R>(
        &self,
        consume: impl FnOnce(&Face, ReadStatus) -> R,
    ) -> Result<R, CustodyRefusal> {
        let face = self.face.try_borrow().map_err(|_| CustodyRefusal::Busy)?;
        let face = face.bound()?;
        Ok(consume(
            face.current(),
            ReadStatus {
                source_sequence: face.last_source_sequence(),
                publisher_retained: self.retained.get(),
                refusal: self.refusal.get(),
            },
        ))
    }

    /// Claim the actual local storage once. Inert backing remains unreadable
    /// until Root binds its admitted scope; this claim supplies no authority.
    /// Closing publication custody never grants a replacement producer.
    pub fn claim(&self) -> Result<(Publisher<'_>, Reader<'_>), CustodyRefusal> {
        if self.claimed.replace(true) {
            return Err(CustodyRefusal::AlreadyClaimed);
        }
        self.retained.set(true);
        Ok((Publisher { owner: self }, Reader { owner: self }))
    }
}
impl Publisher<'_> {
    #[inline(never)]
    pub fn publish(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        committed_ms: u64,
    ) -> Result<(), CustodyRefusal> {
        let mut face = self.owner.face.try_borrow_mut().map_err(|_| {
            self.owner.refusal.set(Some("face-busy"));
            CustodyRefusal::Busy
        })?;
        let result = face
            .bound_mut()?
            .publish(snapshot, committed_ms)
            .map_err(CustodyRefusal::Publication);
        self.owner
            .refusal
            .set(result.as_ref().err().map(CustodyRefusal::code));
        result
    }
}
impl Drop for Publisher<'_> {
    fn drop(&mut self) {
        // Independent Cell: even a reentrant drop during a read cannot panic or
        // leave a stale producer claim because the Face itself is borrowed.
        self.owner.retained.set(false);
    }
}
impl Reader<'_> {
    /// Root projects its committed name without changing observation custody.
    /// This projection conveys no administrative authority.
    pub fn project_body_name(&self, body_id: &str, name: &str) -> Result<(), CustodyRefusal> {
        self.owner
            .face
            .try_borrow_mut()
            .map_err(|_| CustodyRefusal::Busy)?
            .bound_mut()?
            .project_body_name(body_id, name)
            .map_err(CustodyRefusal::Publication)
    }

    /// Borrow the exact retained revision without advancing its freshness clock.
    /// Mask-local paging may read the same truth repeatedly; the owner advances
    /// semantic age separately through `with_current`. This creates no clock,
    /// caller permit, membership or output authority. Shared reads may nest;
    /// a simultaneous publication or clock update refuses with Busy.
    /// ```compile_fail
    /// use conduit_environment_runtime::publication_face_custody::Reader;
    /// fn leak<'a>(reader: &'a Reader<'a>) -> &'a conduit_face::Face {
    ///     reader.with_retained(|face, _| face).unwrap()
    /// }
    /// ```
    pub fn with_retained<R>(
        &self,
        consume: impl FnOnce(&Face, ReadStatus) -> R,
    ) -> Result<R, CustodyRefusal> {
        self.owner.with_retained(consume)
    }

    /// Age and borrow one sealed revision synchronously. The consumer cannot
    /// retain this borrow across await or return a borrowed Face as R. Reentrant
    /// read/write pressure refuses without blocking, retrying or allocating.
    /// ```compile_fail
    /// use conduit_environment_runtime::publication_face_custody::Reader;
    /// fn leak<'a>(reader: &'a Reader<'a>) -> &'a conduit_face::Face {
    ///     reader.with_current(0, |face, _| face).unwrap()
    /// }
    /// ```
    pub fn with_current<R>(
        &self,
        now_ms: u64,
        consume: impl FnOnce(&Face, ReadStatus) -> R,
    ) -> Result<R, CustodyRefusal> {
        let mut face = self
            .owner
            .face
            .try_borrow_mut()
            .map_err(|_| CustodyRefusal::Busy)?;
        let face = face.bound_mut()?;
        face.at(now_ms).map_err(|reason| {
            self.owner.refusal.set(Some("clock-reversed"));
            CustodyRefusal::Clock(reason)
        })?;
        Ok(consume(
            face.current(),
            ReadStatus {
                source_sequence: face.last_source_sequence(),
                publisher_retained: self.owner.retained.get(),
                refusal: self.owner.refusal.get(),
            },
        ))
    }
}
