//! Canonical evidence identity of an executed checked speech correspondence.
//! This is an evidence packet, not a linguistic IR or commitment/playback grant.
//! Syllable structures and positioned boundary tapes supplied separately from
//! PreparedSpeechCorrespondence are not included; callers must bind those too.
use crate::correspondence::PreparedSpeechCorrespondence;
use alloc::vec::Vec;
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};

pub const CHECKED_CARRIER_IDENTITY_DOMAIN: &str = "conduit.speech/checked-carrier@1";

/// Length-prefix every exact canonical Native component, preserving list order.
/// The two list counts separate phone and phoneme binding/transcription pairs.
pub fn canonical_carrier_bytes(
    original: &PreparedSpeechCorrespondence<'_, '_>,
) -> Result<Vec<u8>, NativeBindingRefusal> {
    let inventory = original.inventory();
    let mut bytes = Vec::new();
    append(&mut bytes, original.intent().clone())?;
    append(&mut bytes, original.phonemes().clone())?;
    append(&mut bytes, original.phones().clone())?;
    append(&mut bytes, original.text().clone())?;
    append(&mut bytes, original.alignment().clone())?;
    append(&mut bytes, inventory.basis().clone())?;
    append(&mut bytes, inventory.inventory().clone())?;
    append(&mut bytes, inventory.notation().profile().clone())?;
    bytes.extend_from_slice(&(inventory.phones().len() as u64).to_le_bytes());
    for (binding, transcription) in inventory.phones() {
        append(&mut bytes, binding.clone())?;
        append(&mut bytes, transcription.checked_match().clone())?;
    }
    bytes.extend_from_slice(&(inventory.phonemes().len() as u64).to_le_bytes());
    for (binding, transcription) in inventory.phonemes() {
        append(&mut bytes, binding.clone())?;
        append(&mut bytes, transcription.checked_match().clone())?;
    }
    Ok(bytes)
}

pub fn checked_carrier_identity(
    original: &PreparedSpeechCorrespondence<'_, '_>,
) -> Result<[u8; 32], NativeBindingRefusal> {
    Ok(conduit_core::semantic_digest(
        CHECKED_CARRIER_IDENTITY_DOMAIN,
        &canonical_carrier_bytes(original)?,
    ))
}

fn append<T: NativeRustBinding>(bytes: &mut Vec<u8>, value: T) -> Result<(), NativeBindingRefusal> {
    let encoded = value.encode()?;
    bytes.extend_from_slice(&(encoded.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&encoded);
    Ok(())
}
