//! Leases only an actual frozen epoch. One service turn verifies one record;
//! dropping or finishing returns the same owner to its original slot.
use super::state::Active;
use super::*;
use conduit_body::BodyId;
use custody_acceptance::{ClosedOfferDraft, OwnedInspector};
use ed25519_dalek::VerifyingKey;
fn slot<R>(index: u8, action: impl FnOnce(&mut State) -> R) -> Result<R, &'static str> {
    if index == 0 {
        return STATE.lock(|cell| {
            cell.borrow_mut()
                .as_mut()
                .map(action)
                .ok_or("observed-offer-state")
        });
    }
    EPOCHS.lock(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|slots| slots.get_mut(usize::from(index - 1)))
            .map(action)
            .ok_or("observed-offer-slot")
    })
}
pub(crate) struct Inspection {
    slot: u8,
    owned: Option<OwnedInspector<ArchiveRegion>>,
}
impl Inspection {
    pub(crate) fn start(
        index: u8,
        body: &BodyId,
        key: &VerifyingKey,
    ) -> Result<Self, &'static str> {
        let active = slot(index, |state| {
            if !matches!(state, State::Active(active) if active.recovered && active.signed && !active.failed)
            {
                return Err("observed-offer-not-frozen");
            }
            let State::Active(active) = core::mem::replace(state, State::Failed) else {
                unreachable!()
            };
            Ok(active)
        })??;
        match OwnedInspector::start(active, body, key) {
            Ok(owned) => Ok(Self {
                slot: index,
                owned: Some(owned),
            }),
            Err((active, error)) => {
                restore(index, active);
                Err(error)
            }
        }
    }
    pub(crate) fn step(&mut self) -> Result<bool, &'static str> {
        let owned = self.owned.as_mut().ok_or("observed-offer-consumed")?;
        with_work(|work| owned.step(work))
    }
    pub(crate) fn finish(self) -> Result<ClosedOfferDraft, &'static str> {
        self.finish_with(Ok)
    }
    /// Keep the actual frozen owner leased through the synchronous Root check.
    /// No callback, accepted bytes, or decoded offer can replace this owner.
    pub(crate) fn finish_with<R>(
        self,
        check: impl FnOnce(ClosedOfferDraft) -> Result<R, &'static str>,
    ) -> Result<R, &'static str> {
        let mut closed = self.finish_owned()?;
        check(closed.draft.take().expect("inspected closed draft"))
    }
    pub(crate) fn finish_owned(mut self) -> Result<ClosedEpoch, &'static str> {
        let owned = self.owned.take().ok_or("observed-offer-consumed")?;
        let (active, result) = owned.finish();
        let mut closed = ClosedEpoch {
            slot: self.slot,
            state: Some(State::Active(active)),
            draft: None,
        };
        closed.draft = Some(result?);
        Ok(closed)
    }
}
/// Actual immutable journal plus its inspected draft. Drop restores ownership;
/// decoded offers and signed receipts cannot construct this lease.
pub(crate) struct ClosedEpoch {
    slot: u8,
    state: Option<State>,
    draft: Option<ClosedOfferDraft>,
}
impl ClosedEpoch {
    pub(crate) fn draft(&self) -> &ClosedOfferDraft {
        self.draft.as_ref().expect("closed draft")
    }
    pub(crate) fn sign_offer<R>(
        mut self,
        sign: impl FnOnce(ClosedOfferDraft) -> Result<R, &'static str>,
    ) -> Result<R, &'static str> {
        sign(self.draft.take().expect("inspected closed draft"))
    }
    pub(crate) fn identity(&self) -> [u8; 32] {
        let Some(State::Active(active)) = &self.state else {
            unreachable!("closed active owner")
        };
        active
            .journal
            .inspect_storage(custody_journal::WholeEpoch::identity)
    }
    /// No write occurs here. Actual signed layout custody and exact closed
    /// acceptance are checked before handing the same physical owner to a job.
    pub(crate) fn prepare_reclaim(
        mut self,
        accepted: &[u8; custody_journal::ACCEPTANCE],
        receipt: &custody_journal::layout::Receipt,
        body: &BodyId,
        key: &VerifyingKey,
    ) -> Result<(ReclaimSlot, custody_journal::PreparedReclaim<ArchiveRegion>), &'static str> {
        if !self.draft().matches_accepted(accepted, body, key) {
            return Err("observed-reclaim-acceptance");
        }
        let identity = self.identity();
        let prepared = with_work(|work| {
            let work: &mut [u8; custody_journal::WORK] =
                work.try_into().map_err(|_| "observed-reclaim-workspace")?;
            let Some(State::Active(active)) = &self.state else {
                unreachable!()
            };
            let witness = active
                .journal
                .inspect_storage(|storage| receipt.verify(body, key, storage))
                .map_err(|_| "observed-reclaim-layout")?;
            let Some(State::Active(active)) = self.state.take() else {
                unreachable!()
            };
            let verify = |bytes: &[u8; custody_journal::ACCEPTANCE]| {
                custody_acceptance::verify_accepted(bytes, body, key)
            };
            let custody = match custody_journal::Custody::open_established(
                active.journal.into_storage(),
                witness,
                work,
                verify,
            ) {
                Ok(custody) => custody,
                Err((storage, _)) => {
                    self.state = Some(State::FrozenRegion(storage));
                    return Err("observed-reclaim-open");
                }
            };
            match custody.prepare_begin(accepted, verify, |storage, bytes| {
                use custody_journal::WholeEpoch;
                if storage.identity() == identity && bytes == accepted {
                    Ok(())
                } else {
                    Err(custody_journal::Error::Region)
                }
            }) {
                Ok(prepared) => Ok(prepared),
                Err((custody, _)) => {
                    self.state = Some(State::Pending(custody));
                    Err("observed-reclaim-prepare")
                }
            }
        })?;
        Ok((
            ReclaimSlot {
                index: self.slot,
                identity,
            },
            prepared,
        ))
    }
}
impl Drop for ClosedEpoch {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            restore_state(self.slot, state);
        }
    }
}
/// Root retains this destination while the actual owner resides in the wait
/// queue. Neither a slot integer nor incoming ACK can construct this token.
#[must_use]
pub(crate) struct ReclaimSlot {
    index: u8,
    identity: [u8; 32],
}
impl ReclaimSlot {
    pub(crate) fn retained(self, custody: custody_journal::Custody<ArchiveRegion>) {
        assert_eq!(
            custody.identity(),
            self.identity,
            "same reclaimed physical owner"
        );
        restore_state(self.index, State::Pending(custody));
    }
    pub(crate) fn erased(self, proof: custody_journal::ErasedData<ArchiveRegion>) {
        assert_eq!(
            proof.identity(),
            self.identity,
            "same erased physical owner"
        );
        restore_state(self.index, rearm_reclaimed(proof));
    }
}
fn restore_state(index: u8, state: State) {
    slot(index, |destination| {
        assert!(
            matches!(destination, State::Failed),
            "frozen epoch lease replaced"
        );
        *destination = state;
    })
    .expect("frozen epoch slot remains installed");
}
fn restore(index: u8, active: Active<ArchiveRegion>) {
    restore_state(index, State::Active(active));
}
impl Drop for Inspection {
    fn drop(&mut self) {
        if let Some(owned) = self.owned.take() {
            let (active, _) = owned.finish();
            restore(self.slot, active);
        }
    }
}

/// Read-only scheduling hint. Taking custody still checks the actual slot.
pub(crate) fn pending(index: u8) -> bool {
    slot(index, |state| matches!(state, State::Pending(_))).unwrap_or(false)
}
/// Resume only a committed transaction already authenticated by ordinary Boot.
/// No received ACK, new layout, or fresh signature can create this entrance.
pub(crate) fn resume_pending(
    index: u8,
) -> Result<(ReclaimSlot, custody_journal::PendingReclaim<ArchiveRegion>), &'static str> {
    let state = slot(index, |state| {
        if !matches!(state, State::Pending(_)) {
            return Err("observed-resume-not-pending");
        }
        Ok(core::mem::replace(state, State::Failed))
    })??;
    let mut lease = ClosedEpoch {
        slot: index,
        state: Some(state),
        draft: None,
    };
    with_work(|work| {
        let work: &mut [u8; custody_journal::WORK] =
            work.try_into().map_err(|_| "observed-resume-workspace")?;
        let Some(State::Pending(custody)) = lease.state.take() else {
            unreachable!()
        };
        let identity = custody.identity();
        match custody.resume_staged(work) {
            Ok(pending) => Ok((ReclaimSlot { index, identity }, pending)),
            Err((custody, _)) => {
                lease.state = Some(State::Pending(custody));
                Err("observed-resume-uncommitted")
            }
        }
    })
}
