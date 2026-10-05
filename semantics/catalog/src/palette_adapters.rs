//! Reviewed semantic conversions; unary shape alone never makes a Kind an adapter.
use crate::{GearPalette, QUANTITY_INFO_WRAP_KIND};
use alloc::vec::Vec;
use conduit_core::{kind_id, ConnectionTrack, KindId, KindIdentity, PortDescriptor, PortDirection};

/// One explicit conversion Gear, not implicit connectivity or a selected Back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteAdapterSuggestion {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub input: PortDescriptor,
    pub output: PortDescriptor,
}

impl GearPalette {
    /// Suggest only reviewed conversions whose two exact Cord contracts check.
    /// The caller must still explicitly author the Gear and both Cords.
    pub fn adapter_suggestions(
        &self,
        source: &PortDescriptor,
        sink: &PortDescriptor,
    ) -> Vec<PaletteAdapterSuggestion> {
        if source.direction != PortDirection::Output || sink.direction != PortDirection::Input {
            return Vec::new();
        }
        // The domain contract explicitly preserves Quantity in a structured leaf.
        // No other unary Kind is inferred to be a conversion.
        let Some(entry) = self.find(&kind_id(QUANTITY_INFO_WRAP_KIND)) else {
            return Vec::new();
        };
        let reviewed = crate::quantity_info_wrap_semantic_contract();
        if entry.kind_contract_revision != reviewed.kind_contract_revision
            || entry.front != reviewed.checked_front()
        {
            return Vec::new();
        }
        let input = &reviewed.inputs[0];
        let output = &reviewed.outputs[0];
        if conduit_plot::validate_connection_contract(source, input, ConnectionTrack::Payload)
            .is_err()
            || conduit_plot::validate_connection_contract(output, sink, ConnectionTrack::Payload)
                .is_err()
        {
            return Vec::new();
        }
        alloc::vec![PaletteAdapterSuggestion {
            kind_id: reviewed.kind_id,
            kind_contract_revision: reviewed.kind_contract_revision,
            input: input.clone(),
            output: output.clone(),
        }]
    }
}
