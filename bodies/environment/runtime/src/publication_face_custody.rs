//! Process-local single-executor custody for one prepared producer Face.
//! IDs and these handles confer no Body membership, routing or hardware grant.
use crate::{
    publication::PublicationSnapshot,
    retained_publication_face::{RetainedPublicationFace, RetainedPublicationRefusal},
};
use conduit_environment_face::PublicationRefusal;
use conduit_face::Face;
use core::cell::{Cell, RefCell};

pub struct LocalPublicationFace {
    face: RefCell<RetainedPublicationFace>,
    claimed: Cell<bool>,
    generation: Cell<u64>,
    retained: Cell<bool>,
    refusal: Cell<Option<&'static str>>,
}
#[derive(Debug, PartialEq, Eq)]
pub enum CustodyRefusal {
    AlreadyClaimed,
    Scope,
    GenerationExhausted,
    Busy,
    Publication(RetainedPublicationRefusal),
    Clock(PublicationRefusal),
}
impl CustodyRefusal {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Scope => "face-custody-scope",
            Self::GenerationExhausted => "face-generation-exhausted",
            Self::AlreadyClaimed => "face-already-claimed",
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
    generation: u64,
}
/// One non-Clone reader, not an admitted Mask caller or remote Line.
/// ```compile_fail
/// use conduit_environment_runtime::publication_face_custody::Reader;
/// fn require_send<T: Send>() {}
/// require_send::<Reader<'static>>();
/// ```
pub struct Reader<'a> {
    owner: &'a LocalPublicationFace,
    generation: u64,
}
impl LocalPublicationFace {
    pub fn new(face: RetainedPublicationFace) -> Self {
        Self {
            face: RefCell::new(face),
            claimed: Cell::new(false),
            generation: Cell::new(0),
            retained: Cell::new(false),
            refusal: Cell::new(None),
        }
    }
    /// Claim once, after the enclosing Root admits actual backing and scope.
    /// Closing publication custody never grants a replacement producer.
    pub fn claim(&self) -> Result<(Publisher<'_>, Reader<'_>), CustodyRefusal> {
        if self.claimed.replace(true) {
            return Err(CustodyRefusal::AlreadyClaimed);
        }
        self.retained.set(true);
        Ok((
            Publisher {
                owner: self,
                generation: self.generation.get(),
            },
            Reader {
                owner: self,
                generation: self.generation.get(),
            },
        ))
    }
}
impl Publisher<'_> {
    #[inline(never)]
    pub fn publish(
        &mut self,
        snapshot: PublicationSnapshot<'_>,
        committed_ms: u64,
    ) -> Result<(), CustodyRefusal> {
        self.check()?;
        let mut face = self.owner.face.try_borrow_mut().map_err(|_| {
            self.owner.refusal.set(Some("face-busy"));
            CustodyRefusal::Busy
        })?;
        let result = face
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
        if self.generation == self.owner.generation.get() {
            self.owner.retained.set(false);
        }
    }
}
impl Reader<'_> {
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
        self.check()?;
        let face = self
            .owner
            .face
            .try_borrow()
            .map_err(|_| CustodyRefusal::Busy)?;
        Ok(consume(
            face.current(),
            ReadStatus {
                source_sequence: face.last_source_sequence(),
                publisher_retained: self.owner.retained.get(),
                refusal: self.owner.refusal.get(),
            },
        ))
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
        self.check()?;
        let mut face = self
            .owner
            .face
            .try_borrow_mut()
            .map_err(|_| CustodyRefusal::Busy)?;
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

/// Exclusive returned backing. Dropping it leaves the owner closed; only a
/// separately prepared Face can replace the old finite stream.
pub struct Returned<'a> {
    owner: &'a LocalPublicationFace,
    generation: u64,
}
impl<'a> LocalPublicationFace {
    /// Consume both actual handles. No replacement admission is inferred here.
    #[allow(clippy::type_complexity)]
    pub fn reunite(
        &'a self,
        publisher: Publisher<'a>,
        reader: Reader<'a>,
    ) -> Result<Returned<'a>, (Publisher<'a>, Reader<'a>, CustodyRefusal)> {
        let checked = (|| {
            if !core::ptr::eq(self, publisher.owner)
                || !core::ptr::eq(self, reader.owner)
                || publisher.generation != self.generation.get()
                || reader.generation != self.generation.get()
                || !self.retained.get()
            {
                return Err(CustodyRefusal::Scope);
            }
            let next = self
                .generation
                .get()
                .checked_add(1)
                .ok_or(CustodyRefusal::GenerationExhausted)?;
            let _face = self
                .face
                .try_borrow_mut()
                .map_err(|_| CustodyRefusal::Busy)?;
            self.generation.set(next);
            self.retained.set(false);
            Ok(next)
        })();
        match checked {
            Ok(generation) => Ok(Returned {
                owner: self,
                generation,
            }),
            Err(error) => Err((publisher, reader, error)),
        }
    }
}
impl Publisher<'_> {
    fn check(&self) -> Result<(), CustodyRefusal> {
        if self.generation != self.owner.generation.get() || !self.owner.retained.get() {
            return Err(CustodyRefusal::Scope);
        }
        Ok(())
    }
}
impl Reader<'_> {
    fn check(&self) -> Result<(), CustodyRefusal> {
        if self.generation != self.owner.generation.get() {
            return Err(CustodyRefusal::Scope);
        }
        Ok(())
    }
}
impl<'a> Returned<'a> {
    // Failure must return the exact prepared backing without allocating.
    #[allow(clippy::type_complexity, clippy::result_large_err)]
    pub fn replace(
        self,
        face: RetainedPublicationFace,
    ) -> Result<(Publisher<'a>, Reader<'a>), (Self, RetainedPublicationFace, CustodyRefusal)> {
        if self.generation != self.owner.generation.get() || self.owner.retained.get() {
            return Err((self, face, CustodyRefusal::Scope));
        }
        let Ok(mut current) = self.owner.face.try_borrow_mut() else {
            return Err((self, face, CustodyRefusal::Busy));
        };
        *current = face;
        self.owner.refusal.set(None);
        self.owner.retained.set(true);
        Ok((
            Publisher {
                owner: self.owner,
                generation: self.generation,
            },
            Reader {
                owner: self.owner,
                generation: self.generation,
            },
        ))
    }
}

#[cfg(test)]
mod renewal_tests {
    use super::*;
    fn face(label: &str) -> RetainedPublicationFace {
        RetainedPublicationFace::prepare(
            conduit_face::FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: Some("source/test".into()),
                checked_plot_id: Some("checked/test".into()),
                expanded_plot_id: Some("expanded/test".into()),
                plan_id: Some("plan/test".into()),
                active_play_id: Some(label.into()),
                sign_ids: alloc::vec![],
            },
            3,
            100,
        )
        .unwrap()
    }
    #[test]
    fn repeated_finite_return_has_fresh_basis_and_fences_all_old_operations() {
        let owner = LocalPublicationFace::new(face("first"));
        let (mut publisher, mut reader) = owner.claim().unwrap();
        for index in 0..4 {
            let mut stale_publisher = Publisher {
                owner: &owner,
                generation: owner.generation.get(),
            };
            let stale_reader = Reader {
                owner: &owner,
                generation: owner.generation.get(),
            };
            let returned = owner.reunite(publisher, reader).ok().unwrap();
            let next = returned.replace(face("next")).ok().unwrap();
            publisher = next.0;
            reader = next.1;
            assert_eq!(owner.generation.get(), index + 1);
            assert!(matches!(
                stale_reader.with_current(0, |_, _| ()),
                Err(CustodyRefusal::Scope)
            ));
            assert!(matches!(
                stale_reader.with_retained(|_, _| ()),
                Err(CustodyRefusal::Scope)
            ));
            assert!(matches!(
                stale_publisher.publish(
                    PublicationSnapshot {
                        channel: crate::publication::PublicationChannel::Reading,
                        sequence: 1,
                        canonical: &[]
                    },
                    0
                ),
                Err(CustodyRefusal::Scope)
            ));
            drop(stale_publisher);
            assert!(reader.with_retained(|_, s| s.publisher_retained).unwrap());
            reader
                .with_retained(|f, _| {
                    assert_eq!(f.basis.active_play_id.as_ref().unwrap().as_str(), "next")
                })
                .unwrap();
        }
    }
    #[test]
    fn mismatch_busy_overflow_and_replace_busy_return_actual_owners() {
        let owner = LocalPublicationFace::new(face("first"));
        let other = LocalPublicationFace::new(face("other"));
        let (publisher, reader) = owner.claim().unwrap();
        let (other_publisher, other_reader) = other.claim().unwrap();
        let (publisher, other_reader, error) =
            owner.reunite(publisher, other_reader).err().unwrap();
        assert_eq!(error, CustodyRefusal::Scope);
        let borrow = owner.face.borrow();
        let (publisher, reader, error) = owner.reunite(publisher, reader).err().unwrap();
        assert_eq!(error, CustodyRefusal::Busy);
        assert!(owner.retained.get());
        assert_eq!(owner.generation.get(), 0);
        drop(borrow);
        let returned = owner.reunite(publisher, reader).ok().unwrap();
        let borrow = owner.face.borrow();
        let (returned, replacement, error) = returned.replace(face("next")).err().unwrap();
        assert_eq!(error, CustodyRefusal::Busy);
        drop(borrow);
        let (publisher, reader) = returned.replace(replacement).ok().unwrap();
        // Private synthetic boundary fixture: no public entrance can forge it.
        owner.generation.set(u64::MAX);
        drop(publisher);
        owner.retained.set(true);
        let publisher = Publisher {
            owner: &owner,
            generation: u64::MAX,
        };
        let reader = Reader {
            generation: u64::MAX,
            ..reader
        };
        let (publisher, _, error) = owner.reunite(publisher, reader).err().unwrap();
        assert_eq!(error, CustodyRefusal::GenerationExhausted);
        assert!(owner.retained.get());
        drop(publisher);
        drop((other_publisher, other_reader));
    }
}
