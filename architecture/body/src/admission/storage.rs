//! Fallible compaction of retained inline storage, independent of admission law.
use super::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AdmissionStorageCompactionRefusal {
    Capacity,
}

impl AdmissionManager {
    /// Share equal immutable identity bytes within this manager, without
    /// normalizing values or changing any receipt, key, attempt, or ordering.
    /// Wire decoding does not intern Arc-backed IDs; this bounded pass needs no
    /// index, allocation, authority check, or new global identity lifetime.
    /// Oversized decoded collections remain unchanged for ordinary validation.
    #[inline(never)]
    pub fn share_equal_identity_storage(&mut self) {
        if self.pending.len() > MAX_PENDING_ADMISSIONS
            || self.invitations.len() > MAX_SPAWN_INVITATIONS
            || self.continuity_keys.len() > crate::MAX_BODY_PARTS
            || self.pending_returns.len() > MAX_PENDING_ADMISSIONS
            || self.receipts.len() > MAX_ADMISSION_RECEIPTS
        {
            return;
        }
        for pending in &mut self.pending {
            if pending.challenge.body_id == self.body_id {
                pending.challenge.body_id = self.body_id.clone();
            }
        }
        for index in 0..self.continuity_keys.len() {
            let (previous, remaining) = self.continuity_keys.split_at_mut(index);
            let key = &mut remaining[0];
            if let Some(prior) = previous.iter().find(|p| p.part_id == key.part_id) {
                key.part_id = prior.part_id.clone();
            }
        }
        let receipts = self.receipts.owned_receipts_mut();
        for index in 0..receipts.len() {
            let (previous, remaining) = receipts.split_at_mut(index);
            let credential = &mut remaining[0].credential;
            if credential.body_id != self.body_id {
                continue;
            }
            credential.body_id = self.body_id.clone();
            let canonical = self
                .continuity_keys
                .iter()
                .find(|key| key.part_id == credential.part_id)
                .map(|key| &key.part_id)
                .or_else(|| {
                    previous
                        .iter()
                        .find(|prior| {
                            prior.credential.body_id == self.body_id
                                && prior.credential.part_id == credential.part_id
                        })
                        .map(|prior| &prior.credential.part_id)
                });
            if let Some(canonical) = canonical {
                credential.part_id = canonical.clone();
            }
        }
        for pending in &mut self.pending_returns {
            if pending.challenge.body_id != self.body_id {
                continue;
            }
            pending.challenge.body_id = self.body_id.clone();
            let canonical = self
                .continuity_keys
                .iter()
                .find(|key| key.part_id == pending.challenge.part_id)
                .map(|key| &key.part_id)
                .or_else(|| {
                    self.receipts
                        .owned_receipts()
                        .iter()
                        .find(|r| {
                            r.credential.body_id == self.body_id
                                && r.credential.part_id == pending.challenge.part_id
                        })
                        .map(|r| &r.credential.part_id)
                });
            if let Some(canonical) = canonical {
                pending.challenge.part_id = canonical.clone();
            }
        }
    }

    /// Remove spare inline collection capacity without retiring any admission,
    /// key, invitation, return, or receipt. Order and nested allocations remain
    /// unchanged. Capacity is storage accounting, never an admission grant.
    ///
    /// All needed empty replacement backings are reserved before moving any
    /// record. A capacity refusal leaves both contents and capacities unchanged.
    /// Temporary replacement storage overlaps the original manager until commit.
    #[inline(never)]
    pub fn try_compact_storage(&mut self) -> Result<(), AdmissionStorageCompactionRefusal> {
        let pending = replacement(&self.pending)?;
        let invitations = replacement(&self.invitations)?;
        let keys = replacement(&self.continuity_keys)?;
        let returns = replacement(&self.pending_returns)?;
        self.receipts.try_compact_spare()?;
        replace(&mut self.pending, pending);
        replace(&mut self.invitations, invitations);
        replace(&mut self.continuity_keys, keys);
        replace(&mut self.pending_returns, returns);
        Ok(())
    }
}

pub(super) fn replacement<T>(
    records: &Vec<T>,
) -> Result<Option<Vec<T>>, AdmissionStorageCompactionRefusal> {
    if records.capacity() == records.len() {
        return Ok(None);
    }
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(records.len())
        .map_err(|_| AdmissionStorageCompactionRefusal::Capacity)?;
    Ok(Some(replacement))
}

pub(super) fn replace<T>(records: &mut Vec<T>, replacement: Option<Vec<T>>) {
    if let Some(mut replacement) = replacement {
        // Reservation already guarantees enough capacity for every original
        // element. append moves them without cloning or allocating.
        replacement.append(records);
        *records = replacement;
    }
}
