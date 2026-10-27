//! Maintenance-only reclamation of an immutable signed epoch. The running
//! sampler never enters this service or waits for a remote acknowledgement.
use super::*;
use custody_acceptance::{self as acceptance, wire};
use custody_journal::{self as journal, WholeEpoch};

fn locked_slot<R>(
    slot: u8,
    action: impl FnOnce(&mut State) -> Result<R, &'static str>,
) -> Result<R, &'static str> {
    if slot == 0 {
        return STATE.lock(|cell| action(cell.borrow_mut().as_mut().ok_or("observed-cold-state")?));
    }
    EPOCHS.lock(|cell| {
        let mut epochs = cell.borrow_mut();
        action(
            epochs
                .as_mut()
                .and_then(|epochs| epochs.get_mut(usize::from(slot - 1)))
                .ok_or("observed-cold-slot")?,
        )
    })
}
struct SlotLease {
    slot: u8,
    state: Option<State>,
}
impl Drop for SlotLease {
    fn drop(&mut self) {
        if let Some(state) = self.state.take() {
            let _ = locked_slot(self.slot, |slot| {
                *slot = state;
                Ok(())
            });
        }
    }
}
fn at<R>(
    slot: u8,
    action: impl FnOnce(&mut State) -> Result<R, &'static str>,
) -> Result<R, &'static str> {
    // Only the synchronous maintenance service mutates these slots. Readers
    // see Failed while its affine owner is leased; crypto and flash operations
    // run after the critical section has ended.
    let state = locked_slot(slot, |state| Ok(core::mem::replace(state, State::Failed)))?;
    let mut lease = SlotLease {
        slot,
        state: Some(state),
    };
    action(lease.state.as_mut().ok_or("observed-cold-lease")?)
}
fn inspect(state: &mut State) -> Result<acceptance::ClosedOfferDraft, &'static str> {
    let State::Active(active) = state else {
        return Err("observed-cold-not-closed");
    };
    let (body, root) = crate::native_restart::observed::pinned_scope()?;
    with_work(|work| acceptance::inspect(active, &body, &root, work))
}
pub(crate) fn offer(slot: u8) -> Result<wire::Offer<'static>, &'static str> {
    at(slot, |state| {
        crate::native_restart::observed::sign_offer(inspect(state)?)
    })
}

/// An error retains physical custody in FrozenRegion or Pending. Neither is
/// eligible for local append. A reset reopens the actual durable transaction.
pub(crate) fn acknowledge(request: &wire::Request<'_>) -> Result<u8, &'static str> {
    request.validate()?;
    for slot in 0..=5 {
        let matching = at(slot, |state| {
            Ok(inspect(state).is_ok_and(|draft| draft.transcript() == request.offer.0))
        })?;
        if !matching {
            continue;
        }
        return at(slot, |state| {
            let draft = inspect(state)?;
            let accepted = crate::native_restart::observed::acknowledge(&draft, request)?;
            let (body, root) = crate::native_restart::observed::pinned_scope()?;
            let State::Active(active) = core::mem::replace(state, State::Failed) else {
                unreachable!()
            };
            let identity = active.journal.inspect_storage(WholeEpoch::identity);
            let durable = crate::native_restart::observed::read_layout_receipt(&identity);
            // Retain the actual owner before any signature/checkpoint refusal.
            let receipt = match durable {
                Ok(Some(receipt)) => {
                    *state = State::FrozenRegion(active.journal.into_storage());
                    receipt
                }
                Ok(None) => {
                    let preflight =
                        journal::layout::preflight(active.journal, &body, &root.to_bytes());
                    let (storage, layout) = match preflight {
                        Ok(value) => value,
                        Err((storage, _)) => {
                            *state = State::FrozenRegion(storage);
                            return Err("observed-layout-preflight");
                        }
                    };
                    *state = State::FrozenRegion(storage);
                    let signed = crate::native_restart::observed::sign_layout(layout)?;
                    crate::native_restart::observed::persist_layout_receipt(signed)?
                }
                Err(error) => {
                    *state = State::FrozenRegion(active.journal.into_storage());
                    return Err(error);
                }
            };
            let State::FrozenRegion(storage) = state else {
                unreachable!()
            };
            let witness = match receipt.verify(&body, &root, storage) {
                Ok(witness) => witness,
                Err(_) => return Err("observed-layout-verification"),
            };
            with_work(|work| {
                let work: &mut [u8; journal::WORK] =
                    work.try_into().map_err(|_| "observed-cold-workspace")?;
                let State::FrozenRegion(storage) = core::mem::replace(state, State::Failed) else {
                    unreachable!()
                };
                let verify = |bytes: &[u8; journal::ACCEPTANCE]| {
                    acceptance::verify_accepted(bytes, &body, &root)
                };
                let custody = match journal::Custody::open_established(
                    storage, witness, &mut *work, verify,
                ) {
                    Ok(custody) => custody,
                    Err((storage, _)) => {
                        *state = State::FrozenRegion(storage);
                        return Err("observed-cold-open");
                    }
                };
                // `accepted` was constructed above from this exact immutable
                // journal and canonical persisted admitted-message verification.
                let erased = match custody.begin(&accepted, work, verify, |storage, bytes| {
                    if storage.identity() == identity && bytes == &accepted {
                        Ok(())
                    } else {
                        Err(journal::Error::Region)
                    }
                }) {
                    Ok(erased) => erased,
                    Err((custody, _)) => {
                        *state = State::Pending(custody);
                        return Err("observed-cold-reclaim");
                    }
                };
                *state = State::FrozenRegion(ArchiveRegion::from_reclaimed(erased));
                Ok(slot)
            })
        });
    }
    Err("observed-cold-offer-mismatch")
}

/// Continue only an already committed Root-endorsed reclaim transaction.
/// No incoming command, layout receipt, or owner key substitutes for it.
pub(crate) fn resume_pending() -> Result<(), &'static str> {
    for slot in 0..=5 {
        at(slot, |state| {
            if !matches!(state, State::Pending(_)) {
                return Ok(());
            }
            with_work(|work| {
                let work: &mut [u8; journal::WORK] =
                    work.try_into().map_err(|_| "observed-cold-workspace")?;
                let State::Pending(custody) = core::mem::replace(state, State::Failed) else {
                    unreachable!()
                };
                match custody.resume(work) {
                    Ok(erased) => {
                        *state = State::FrozenRegion(ArchiveRegion::from_reclaimed(erased));
                        Ok(())
                    }
                    Err((custody, _)) => {
                        *state = State::Pending(custody);
                        Err("observed-cold-resume")
                    }
                }
            })
        })?;
    }
    Ok(())
}
