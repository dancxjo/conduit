//! Leases only an actual frozen epoch. One service turn verifies one record;
//! dropping or finishing returns the same owner to its original slot.
use super::*;
use super::state::Active;
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
    pub(crate) fn finish(mut self) -> Result<ClosedOfferDraft, &'static str> {
        let owned = self.owned.take().ok_or("observed-offer-consumed")?;
        let (active, result) = owned.finish();
        restore(self.slot, active);
        result
    }
}
fn restore(index: u8, active: Active<ArchiveRegion>) {
    // Only this lease can own a frozen slot marked Failed. Rotation selects
    // Cold slots; current append owns a different, non-recovered Active slot.
    slot(index, |state| {
        assert!(
            matches!(state, State::Failed),
            "frozen epoch lease replaced"
        );
        *state = State::Active(active);
    })
    .expect("frozen epoch slot remains installed");
}
impl Drop for Inspection {
    fn drop(&mut self) {
        if let Some(owned) = self.owned.take() {
            let (active, _) = owned.finish();
            restore(self.slot, active);
        }
    }
}
