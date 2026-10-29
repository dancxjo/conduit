//! Fallible compaction of retained inline storage, independent of admission law.
use super::*;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AdmissionStorageCompactionRefusal {
    Capacity,
}

impl AdmissionManager {
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
        let receipts = replacement(&self.receipts)?;
        replace(&mut self.pending, pending);
        replace(&mut self.invitations, invitations);
        replace(&mut self.continuity_keys, keys);
        replace(&mut self.pending_returns, returns);
        replace(&mut self.receipts, receipts);
        Ok(())
    }
}

fn replacement<T>(records: &Vec<T>) -> Result<Option<Vec<T>>, AdmissionStorageCompactionRefusal> {
    if records.capacity() == records.len() {
        return Ok(None);
    }
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(records.len())
        .map_err(|_| AdmissionStorageCompactionRefusal::Capacity)?;
    Ok(Some(replacement))
}

fn replace<T>(records: &mut Vec<T>, replacement: Option<Vec<T>>) {
    if let Some(mut replacement) = replacement {
        // Reservation already guarantees enough capacity for every original
        // element. append moves them without cloning or allocating.
        replacement.append(records);
        *records = replacement;
    }
}
