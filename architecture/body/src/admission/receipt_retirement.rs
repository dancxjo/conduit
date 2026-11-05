//! Superseded admission evidence remains an explicit durable-custody obligation.
use super::*;

/// Exact evidence selected for external custody, never an IO acknowledgment.
/// Preparing this value does not change the manager or release hot capacity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdmissionReceiptRetirement {
    body_id: BodyId,
    issued_receipts: u64,
    receipts: Vec<AdmissionReceipt>,
}

impl AdmissionReceiptRetirement {
    pub fn body_id(&self) -> &BodyId {
        &self.body_id
    }

    pub fn receipts(&self) -> &[AdmissionReceipt] {
        &self.receipts
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReceiptRetirementRefusal {
    InvalidHistory,
    SequenceExhausted,
    StaleSelection,
}

impl AdmissionManager {
    // Keep issuance independent of hot-vector length after evidence retirement.
    // Old serialized managers have retired_count=0 and retain their old ordinal.
    pub(super) fn receipt_ordinal(&self) -> Result<u64, AdmissionRefusal> {
        let ordinal = self
            .retired_receipt_count
            .checked_add(self.receipts.len() as u64)
            .ok_or(AdmissionRefusal::ReceiptSequenceExhausted)?;
        ordinal
            .checked_add(1)
            .ok_or(AdmissionRefusal::ReceiptSequenceExhausted)?;
        Ok(ordinal)
    }

    /// Select only middle receipts for each Part: its first replay fence and
    /// latest credential stay hot, including the latest potentially lost reply.
    /// At most 32 existing receipts are scanned; preparation overlaps one cloned
    /// subset with the manager. No archive capacity or IO is implied.
    pub fn prepare_receipt_retirement(
        &self,
    ) -> Result<Option<AdmissionReceiptRetirement>, ReceiptRetirementRefusal> {
        if self.receipts.len() > MAX_ADMISSION_RECEIPTS {
            return Err(ReceiptRetirementRefusal::InvalidHistory);
        }
        let issued_receipts = self
            .retired_receipt_count
            .checked_add(self.receipts.len() as u64)
            .ok_or(ReceiptRetirementRefusal::SequenceExhausted)?;
        let mut selected = Vec::new();
        for (index, receipt) in self.receipts.iter().enumerate() {
            if receipt.credential.body_id != self.body_id
                || self.receipts[..index].iter().any(|previous| {
                    previous.admission_id == receipt.admission_id
                        || previous.credential.credential_id == receipt.credential.credential_id
                        || (previous.credential.part_id == receipt.credential.part_id
                            && previous.credential.host_id != receipt.credential.host_id)
                })
            {
                return Err(ReceiptRetirementRefusal::InvalidHistory);
            }
            let same_part =
                |other: &AdmissionReceipt| other.credential.part_id == receipt.credential.part_id;
            if self.receipts[..index].iter().any(same_part)
                && self.receipts[index + 1..].iter().any(same_part)
            {
                selected.push(receipt.clone());
            }
        }
        Ok(
            (!selected.is_empty()).then_some(AdmissionReceiptRetirement {
                body_id: self.body_id.clone(),
                issued_receipts,
                receipts: selected,
            }),
        )
    }

    /// Confirm that the caller has durably retained this exact evidence before
    /// releasing it. Core performs no IO: the owner must put the evidence and
    /// resulting manager in its recoverable transaction, committing evidence
    /// before the smaller manager. Transport delivery cannot satisfy custody.
    /// A stale, changed or repeated selection refuses without mutation.
    pub fn acknowledge_receipt_custody(
        &mut self,
        retained: &AdmissionReceiptRetirement,
    ) -> Result<(), ReceiptRetirementRefusal> {
        if self.prepare_receipt_retirement()?.as_ref() != Some(retained) {
            return Err(ReceiptRetirementRefusal::StaleSelection);
        }
        let count = self
            .retired_receipt_count
            .checked_add(retained.receipts.len() as u64)
            .ok_or(ReceiptRetirementRefusal::SequenceExhausted)?;
        self.receipts
            .retain(|receipt| !retained.receipts.contains(receipt));
        self.retired_receipt_count = count;
        Ok(())
    }
}
