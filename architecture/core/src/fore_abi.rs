//! One canonical Fore ABI fingerprint for Make and bounded runtime validators.
use crate::{CheckedValueContract, ConnectionTrack, QuantityValueProfile};
use sha2::{Digest, Sha256};

/// Semantic and finite queue facts of an already checked Fore. Hashing these
/// facts grants neither ingress, carrier custody nor permission to execute.
pub struct AssignedForeAbi<'a> {
    pub value_kind: &'a str,
    pub value_contract: Option<&'a CheckedValueContract>,
    pub abnormal_kind: Option<&'a str>,
    pub temporal: &'a str,
    pub track: ConnectionTrack,
    pub quantity_profile: QuantityValueProfile,
    pub item_capacity: u16,
    pub byte_capacity: u32,
}
/// Preserve the original version-1 assigned Fore transcript, streaming borrowed
/// contract bytes instead of allocating a second canonical contract/ABI buffer.
pub fn assigned_fore_abi_digest(abi: AssignedForeAbi<'_>) -> [u8; 32] {
    fn field(hash: &mut Sha256, value: &[u8]) {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value);
    }
    let mut hash = Sha256::new();
    hash.update(b"conduit.assigned/local-fore-abi@1\0");
    field(&mut hash, abi.value_kind.as_bytes());
    if let Some(contract) = abi.value_contract {
        hash.update((contract.identity_byte_len() as u64).to_le_bytes());
        contract.visit_identity_bytes(|bytes| hash.update(bytes));
    } else {
        hash.update(0u64.to_le_bytes());
    }
    field(&mut hash, abi.abnormal_kind.unwrap_or("").as_bytes());
    field(&mut hash, abi.temporal.as_bytes());
    hash.update([
        match abi.track {
            ConnectionTrack::Payload => 0,
            ConnectionTrack::NormalClose => 1,
            ConnectionTrack::AbnormalTerminal => 2,
            ConnectionTrack::Quiescence => 3,
        },
        match abi.quantity_profile {
            QuantityValueProfile::Decimal => 0,
            QuantityValueProfile::Exact => 1,
        },
    ]);
    hash.update(abi.item_capacity.to_le_bytes());
    hash.update(abi.byte_capacity.to_le_bytes());
    hash.finalize().into()
}

#[cfg(test)]
mod tests;
