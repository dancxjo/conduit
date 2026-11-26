//! Checked-schema backing model; excludes allocator bookkeeping and root stacks.
//! Bounds are computed from an already admitted ObservationContract. Preparing
//! that contract/catalog itself is a separate owner responsibility.
use crate::{field, ObservationContract};
use conduit_core::{StructuredInfoTypeShape as Shape, QUANTITY_ENCODED_LEN};
use core::mem::size_of;

/// Preparation peak for the exact observation Forms. Schema aliases are charged
/// independently, just as Core's retained_backing_bytes contract specifies.
pub(crate) fn forms_preparation_bound(contract: &ObservationContract) -> Option<usize> {
    let ty = contract.observation_type();
    let prefix = ty.canonical_bytes().ok()?.len();
    let schema = ty.retained_backing_bytes()?;
    let mut bytes = schema.checked_add(prefix)?; // retained validator
    for name in ["temperature", "humidity"] {
        let leaf = field(ty, name).ok()?;
        let prefix = leaf.canonical_bytes().ok()?.len();
        bytes = bytes
            .checked_add(prefix.checked_mul(2)?)?
            .checked_add(5 + QUANTITY_ENCODED_LEN)?;
    }
    let Shape::Record { fields, .. } = ty.shape() else {
        return None;
    };
    // Composer field spans contain two (u32 start,u32 length) pairs. Iterator
    // collection may grow its field table; charge two complete tables.
    let spans = fields.len().checked_mul(2 * 4 * size_of::<u32>())?;
    bytes = bytes
        .checked_add(prefix)?
        .checked_add(crate::MAXIMUM_BYTES as usize)?
        .checked_add(spans)?;
    // During preparation, field canonical prefixes and semantic_digest may
    // coexist with all retained material. One complete schema serialization
    // bounds each child serialization, processed sequentially.
    bytes.checked_add(prefix)
}

impl ObservationContract {
    /// Reachable checked catalog material; excludes inline Self and parser/compiler
    /// construction temporaries. Separate aliases are conservatively charged.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let k = &self.kind;
        if !k.startup_parameters.is_empty()
            || !k.configuration.is_empty()
            || !k.semantic_laws.is_empty()
        {
            return None;
        }
        let mut bytes = self
            .input
            .retained_backing_bytes()?
            .checked_add(self.observation.retained_backing_bytes()?)?
            .checked_add(self.output.retained_backing_bytes()?)?
            .checked_add(k.kind_id.0.capacity())?
            .checked_add(k.kind_contract_revision.0.capacity())?
            .checked_add(
                k.inputs
                    .capacity()
                    .checked_mul(size_of::<conduit_core::PortDescriptor>())?,
            )?
            .checked_add(
                k.outputs
                    .capacity()
                    .checked_mul(size_of::<conduit_core::PortDescriptor>())?,
            )?;
        if let Some((input, output)) = &k.shorthand {
            bytes = bytes
                .checked_add(input.0.capacity())?
                .checked_add(output.0.capacity())?;
        }
        for port in k.inputs.iter().chain(&k.outputs) {
            bytes = bytes
                .checked_add(port.port_id.0.capacity())?
                .checked_add(port.value_kind.0.capacity())?;
            if let Some(abnormal) = &port.abnormal_kind {
                bytes = bytes.checked_add(abnormal.0.capacity())?;
            }
        }
        Some(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_form_preparation_model_covers_retained_capacities() {
        let contract = ObservationContract::prepare().unwrap();
        let forms = crate::PreparedObservationQuantityForms::prepare(&contract).unwrap();
        assert!(
            forms.retained_backing_bytes().unwrap() <= forms_preparation_bound(&contract).unwrap()
        );
        assert!(contract.retained_backing_bytes().unwrap() > 0);
    }
}
