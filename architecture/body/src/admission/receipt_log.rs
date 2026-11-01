//! Lossless resident receipt storage. Compact rows are data, never admission
//! authority; latest credentials stay typed and historical order stays exact.
use super::*;
mod compact;
mod view;
use compact::Compact;
pub use view::{AdmissionReceiptRef, ReceiptCredentialRef, ReceiptIdentityRef};

#[derive(Debug, Clone)]
enum Storage {
    Owned(Vec<AdmissionReceipt>),
    Compact(Compact),
}
#[derive(Debug, Clone)]
pub struct AdmissionReceiptLog(Storage);
/// Actual backing capacities. Hot credentials additionally retain normal nested
/// identity allocations; these counters do not claim a complete heap charge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiptLogStorageUsage {
    pub owned_receipts: usize,
    pub historical_receipts: usize,
    /// Hot rows, packed rows, order bytes, text bytes, dictionary spans.
    pub allocation_capacities: [usize; 5],
}
impl Default for AdmissionReceiptLog {
    fn default() -> Self {
        Self(Storage::Owned(Vec::new()))
    }
}
impl From<Vec<AdmissionReceipt>> for AdmissionReceiptLog {
    fn from(receipts: Vec<AdmissionReceipt>) -> Self {
        Self(Storage::Owned(receipts))
    }
}
impl PartialEq for AdmissionReceiptLog {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().eq(other.iter())
    }
}
impl Eq for AdmissionReceiptLog {}
impl Serialize for AdmissionReceiptLog {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeSeq;
        let mut sequence = serializer.serialize_seq(Some(self.len()))?;
        for receipt in self.iter() {
            sequence.serialize_element(&receipt)?;
        }
        sequence.end()
    }
}
impl<'de> Deserialize<'de> for AdmissionReceiptLog {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        Vec::<AdmissionReceipt>::deserialize(decoder).map(Self::from)
    }
}
impl AdmissionReceiptLog {
    pub fn len(&self) -> usize {
        match &self.0 {
            Storage::Owned(v) => v.len(),
            Storage::Compact(v) => v.order.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Capacity of receipt order entries, not the full representation. Use
    /// storage_usage for all container capacities and hot/cold counts.
    pub fn capacity(&self) -> usize {
        match &self.0 {
            Storage::Owned(v) => v.capacity(),
            Storage::Compact(v) => v.order.capacity(),
        }
    }
    /// Address of the representation's order backing, not an execution identity.
    pub fn as_ptr(&self) -> *const u8 {
        match &self.0 {
            Storage::Owned(v) => v.as_ptr().cast(),
            Storage::Compact(v) => v.order.as_ptr(),
        }
    }
    /// An old cold row has a borrowed view instead of an allocated credential.
    pub fn owned(&self, index: usize) -> Option<&AdmissionReceipt> {
        match &self.0 {
            Storage::Owned(v) => v.get(index),
            Storage::Compact(v) => v
                .order
                .get(index)
                .filter(|i| **i & 128 != 0)
                .and_then(|i| v.hot.get(usize::from(*i & 127))),
        }
    }
    pub fn last(&self) -> Option<&AdmissionReceipt> {
        self.len().checked_sub(1).and_then(|i| self.owned(i))
    }
    pub fn clear(&mut self) {
        self.0 = Storage::Owned(Vec::new());
    }
    pub fn try_reserve_exact(
        &mut self,
        additional: usize,
    ) -> Result<(), AdmissionStorageCompactionRefusal> {
        match &mut self.0 {
            Storage::Owned(v) => v
                .try_reserve_exact(additional)
                .map_err(|_| AdmissionStorageCompactionRefusal::Capacity),
            Storage::Compact(_) => Err(AdmissionStorageCompactionRefusal::Capacity),
        }
    }
    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = AdmissionReceiptRef<'_>> + ExactSizeIterator {
        (0..self.len()).map(|index| self.at(index))
    }
    fn at(&self, index: usize) -> AdmissionReceiptRef<'_> {
        match &self.0 {
            Storage::Owned(v) => AdmissionReceiptRef::from(&v[index]),
            Storage::Compact(v) => v.at(index),
        }
    }
    pub(super) fn owned_receipts(&self) -> &[AdmissionReceipt] {
        match &self.0 {
            Storage::Owned(v) => v,
            Storage::Compact(v) => &v.hot,
        }
    }
    pub(super) fn owned_receipts_mut(&mut self) -> &mut [AdmissionReceipt] {
        match &mut self.0 {
            Storage::Owned(v) => v,
            Storage::Compact(v) => &mut v.hot,
        }
    }
    pub fn latest_completed_credential(&self, part: &PartId) -> Option<&MembershipCredential> {
        self.owned_receipts()
            .iter()
            .rev()
            .find(|r| &r.credential.part_id == part)
            .map(|r| &r.credential)
    }
    pub fn contains_admission(&self, id: &AdmissionId) -> bool {
        self.iter().any(|r| r.admission_id.equals(id.as_str()))
    }
    pub fn try_push(&mut self, receipt: AdmissionReceipt) -> Result<(), AdmissionRefusal> {
        if self.len() >= MAX_ADMISSION_RECEIPTS {
            return Err(AdmissionRefusal::ReceiptCapacityExhausted);
        }
        match &mut self.0 {
            Storage::Owned(v) => {
                v.try_reserve_exact(1)
                    .map_err(|_| AdmissionRefusal::ReceiptCapacityExhausted)?;
                v.push(receipt);
            }
            Storage::Compact(v) => v.try_push(receipt)?,
        }
        Ok(())
    }
    /// Allocate every replacement before moving ownership. Noncanonical digest
    /// strings remain typed/owned. No old receipt is discarded or reconstructed.
    pub fn try_compact_historical(&mut self) -> Result<(), AdmissionStorageCompactionRefusal> {
        let Some(prepared) = compact::prepare(self)? else {
            return Ok(());
        };
        let prior = core::mem::replace(&mut self.0, Storage::Owned(Vec::new()));
        self.0 = Storage::Compact(prepared.finish(prior));
        Ok(())
    }
    pub(super) fn usage(&self) -> crate::VecStorageUsage {
        match &self.0 {
            Storage::Owned(v) => crate::VecStorageUsage::inspect(v),
            Storage::Compact(v) => crate::VecStorageUsage::inspect(&v.order),
        }
    }
    pub fn storage_usage(&self) -> ReceiptLogStorageUsage {
        match &self.0 {
            Storage::Owned(v) => ReceiptLogStorageUsage {
                owned_receipts: v.len(),
                historical_receipts: 0,
                allocation_capacities: [
                    v.capacity() * core::mem::size_of::<AdmissionReceipt>(),
                    0,
                    0,
                    0,
                    0,
                ],
            },
            Storage::Compact(v) => v.usage(),
        }
    }
    /// Container capacities only; hot credentials additionally own their usual
    /// identity storage. Compact text bytes are included, Vec headers excluded.
    pub fn representation_bytes(&self) -> usize {
        match &self.0 {
            Storage::Owned(v) => v.capacity() * core::mem::size_of::<AdmissionReceipt>(),
            Storage::Compact(v) => v.representation_bytes(),
        }
    }
    pub(super) fn try_compact_spare(&mut self) -> Result<(), AdmissionStorageCompactionRefusal> {
        if let Storage::Owned(v) = &mut self.0 {
            let replacement = super::storage::replacement(v)?;
            super::storage::replace(v, replacement);
        }
        Ok(())
    }
}
