//! Closed portable-offer preparation profile, not admission or semantic proof.
//! Known map fields and supported variants are scanned before owned Core decode.
//! JSON parser/inflater and transaction custody remain separate.
use core::mem::size_of;
use sha2::{Digest, Sha256};

#[path = "portable_preparation/json.rs"]
mod json;
#[path = "portable_preparation/records.rs"]
mod records;
#[path = "portable_preparation/semantic.rs"]
mod semantic;
#[path = "portable_preparation/validation.rs"]
mod validation;
use json::R;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Json,
    Unsupported,
    Overflow,
}

/// Facts for the separately qualified borrowed Core validation model.
/// Independent maxima deliberately overbound multiple ValueContracts payloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ValidationShape {
    pub(super) port_symbols: usize,
    pub(super) configuration_keys: usize,
    pub(super) terminal_inputs: usize,
    pub(super) maximum_value_locations: usize,
    pub(super) maximum_value_location_string_bytes: usize,
    pub(super) configuration_kind_bytes: usize,
}

/// A descriptor of typed descendants for these exact bytes, never a byte grant.
/// Outer offer-vector storage, parser/inflater workspace and all product
/// transaction/retirement descendants require independent custody. Validation
/// is a separate pinned, field-specific upper bound exposed by this descriptor.
#[derive(Debug, PartialEq, Eq)]
pub struct PortableOfferPreparation {
    json_sha256: [u8; 32],
    retained_heap_bytes: usize,
    typed_decode_peak_heap_bytes: usize,
    validation: ValidationShape,
}
impl PortableOfferPreparation {
    pub fn validation_heap_upper(&self) -> Result<usize, Refusal> {
        validation::backing(self.validation)
    }

    /// Typed descendants plus the larger of decode reallocation and validation
    /// scratch. Parser/inflater, outer vector and transaction remain excluded.
    pub fn typed_validation_heap_peak(&self) -> Result<usize, Refusal> {
        let validation = self
            .retained_heap_bytes
            .checked_add(self.validation_heap_upper()?)
            .ok_or(Refusal::Overflow)?;
        Ok(self.typed_decode_peak_heap_bytes.max(validation))
    }

    #[cfg(test)]
    fn validation_shape(&self) -> ValidationShape {
        self.validation
    }

    pub fn matches_json(&self, bytes: &[u8]) -> bool {
        self.json_sha256 == <[u8; 32]>::from(Sha256::digest(bytes))
    }
    pub fn retained_heap_bytes(&self) -> usize {
        self.retained_heap_bytes
    }
    pub fn typed_decode_peak_heap_bytes(&self) -> usize {
        self.typed_decode_peak_heap_bytes
    }
}

#[derive(Default)]
struct Layout {
    retained: usize,
    largest_old_vector: usize,
    validation: ValidationShape,
    current_value_locations: usize,
    current_value_location_bytes: usize,
}
impl Layout {
    fn add_fact(value: &mut usize, extra: usize) -> R<()> {
        *value = value.checked_add(extra).ok_or(Refusal::Overflow)?;
        Ok(())
    }
    fn text(&mut self, bytes: usize) -> R<()> {
        self.retained = self.retained.checked_add(bytes).ok_or(Refusal::Overflow)?;
        Ok(())
    }
    // Pinned serde JSON SeqAccess has no size hint. Its ordinary Vec visitor
    // starts empty; pinned Rust grows by doubling and a type-specific minimum.
    fn vector<T>(&mut self, count: usize) -> R<()> {
        if count == 0 {
            return Ok(());
        }
        let size = size_of::<T>();
        if size == 0 {
            return Err(Refusal::Unsupported);
        }
        let minimum = if size == 1 {
            8
        } else if size <= 1024 {
            4
        } else {
            1
        };
        let capacity = count
            .checked_next_power_of_two()
            .ok_or(Refusal::Overflow)?
            .max(minimum);
        let bytes = capacity.checked_mul(size).ok_or(Refusal::Overflow)?;
        // Check the exact native allocation layout, not just usize arithmetic.
        core::alloc::Layout::array::<T>(capacity).map_err(|_| Refusal::Overflow)?;
        self.text(bytes)?;
        if count > minimum {
            self.largest_old_vector = self.largest_old_vector.max(bytes / 2);
        }
        Ok(())
    }
}

/// This narrower carriage accepts map records, canonical unsigned decimal
/// spellings and publisher enum encodings.
/// Field order is arbitrary; unknown/duplicate fields and unsupported variants
/// refuse. IDs remain opaque: no capability, Kind or artifact-name whitelist.
pub fn prepare(json: &[u8]) -> R<PortableOfferPreparation> {
    let raw = json::raw(json)?;
    let mut layout = Layout::default();
    records::offer(raw, &mut layout)?;
    Ok(PortableOfferPreparation {
        json_sha256: Sha256::digest(json).into(),
        retained_heap_bytes: layout.retained,
        validation: layout.validation,
        typed_decode_peak_heap_bytes: layout
            .retained
            .checked_add(layout.largest_old_vector)
            .ok_or(Refusal::Overflow)?,
    })
}

#[cfg(test)]
#[path = "portable_preparation/tests.rs"]
mod tests;
