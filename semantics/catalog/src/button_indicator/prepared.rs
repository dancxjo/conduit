//! Schema and scratch admission for allocation-free button mapping during Play.

use super::{ButtonIndicatorRefusal, BUTTON_TRANSITION_MAXIMUM_BYTES};
use alloc::vec::Vec;
use conduit_core::{
    InfoBool, PreparedStructuredValueValidator, StructuredCanonicalSelection, StructuredSelector,
    UnmatchedVariantDisposition,
};

pub struct PreparedButtonIndicatorMapper {
    validator: PreparedStructuredValueValidator,
    phase: StructuredSelector,
    input_type: Vec<u8>,
    phase_type: Vec<u8>,
    pressed: StructuredSelector,
    unit_type: Vec<u8>,
    scratch: Vec<u8>,
    unit_scratch: Vec<u8>,
}

impl PreparedButtonIndicatorMapper {
    /// Admit the exact schema and all scratch storage before Play starts.
    pub fn new() -> Result<Self, ButtonIndicatorRefusal> {
        let ty = crate::input_button_transition_type();
        let validator =
            PreparedStructuredValueValidator::new(&ty, BUTTON_TRANSITION_MAXIMUM_BYTES as usize)
                .map_err(ButtonIndicatorRefusal::Malformed)?;
        let input_type = ty
            .canonical_bytes()
            .map_err(ButtonIndicatorRefusal::Malformed)?;
        let phase =
            StructuredSelector::field(ty, "phase").map_err(ButtonIndicatorRefusal::Selection)?;
        let phase_type = phase
            .output_type()
            .canonical_bytes()
            .map_err(ButtonIndicatorRefusal::Malformed)?;
        let pressed = StructuredSelector::variant(
            phase.output_type().clone(),
            "pressed",
            UnmatchedVariantDisposition::Drop,
        )
        .map_err(ButtonIndicatorRefusal::Selection)?;
        let unit_type = pressed
            .output_type()
            .canonical_bytes()
            .map_err(ButtonIndicatorRefusal::Malformed)?;
        Ok(Self {
            validator,
            phase,
            input_type,
            phase_type,
            pressed,
            unit_type,
            scratch: Vec::with_capacity(BUTTON_TRANSITION_MAXIMUM_BYTES as usize),
            unit_scratch: Vec::with_capacity(BUTTON_TRANSITION_MAXIMUM_BYTES as usize),
        })
    }

    /// Validate and map borrowed canonical input without constructing owned Info.
    pub fn map(&mut self, canonical: &[u8]) -> Result<InfoBool, ButtonIndicatorRefusal> {
        self.validator
            .validate(canonical)
            .map_err(ButtonIndicatorRefusal::Malformed)?;
        let selected = self
            .phase
            .select_canonical_into(
                canonical,
                &self.input_type,
                &self.phase_type,
                &mut self.scratch,
            )
            .map_err(ButtonIndicatorRefusal::Selection)?;
        if selected != StructuredCanonicalSelection::Matched {
            return Err(ButtonIndicatorRefusal::MissingPhase);
        }
        // The validated exact phase schema has only pressed and released.
        // Leaf payload semantics remain with their Kind, as in the reference mapper.
        match self
            .pressed
            .select_canonical_into(
                &self.scratch,
                &self.phase_type,
                &self.unit_type,
                &mut self.unit_scratch,
            )
            .map_err(ButtonIndicatorRefusal::Selection)?
        {
            StructuredCanonicalSelection::Matched => Ok(InfoBool::TRUE),
            StructuredCanonicalSelection::Unmatched(_) => Ok(InfoBool::FALSE),
        }
    }
}
