//! An ordinary Form admitted to serve as a Body's presentation Mask.
//!
//! Mask is a role, not a second graph language. The checked and expanded Form
//! own topology, typed Cords, finite bounds, cancellation, and terminal
//! semantics. This module only validates the narrow Presentation/interaction/
//! Show boundary and retains its exact ordinary Form identity.

use alloc::string::String;
use conduit_core::{
    FormIdentity, GearId, Plan, PortDescriptor, PortDirection, PortId, PortTemporal,
};
use serde::{Deserialize, Serialize};

use crate::{
    MANIFESTATION_VALUE_KIND, PRESENTATION_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskForm {
    pub form_identity: FormIdentity,
    pub form_name: String,
    pub presentation_input: MaskFormBoundary,
    pub interaction_output: MaskFormBoundary,
    pub show_output: MaskFormBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskFormBoundary {
    pub front_port_id: PortId,
    pub gear_id: GearId,
    pub gear_port_id: PortId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedMaskForm {
    pub mask: MaskForm,
    /// The ordinary immutable Plan for this exact expanded Mask Form.
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskFormError {
    InvalidForm,
    InvalidFront,
    MissingPresentationInput,
    MissingInteractionOutput,
    MissingShowOutput,
    StalePlan,
    MissingPlannedBoundary,
}

impl core::fmt::Display for MaskFormError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid Mask Form: {self:?}")
    }
}

#[cfg(feature = "form-catalog")]
impl MaskForm {
    /// Admit one canonical authoring Form as a Mask. The graph remains an
    /// ordinary Form graph; only its public role boundary is Mask-specific.
    pub fn admit(form: &conduit_form::ExpandedAuthoringForm) -> Result<Self, MaskFormError> {
        form.expanded
            .validate_expansion()
            .map_err(|_| MaskFormError::InvalidForm)?;
        if form.front.inputs().len() != 1
            || form.front.outputs().len() != 2
            || form.input_bindings.len() != 1
            || form.output_bindings.len() != 2
        {
            return Err(MaskFormError::InvalidFront);
        }
        let presentation = required_port(
            form.front.inputs(),
            "presentation",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )
        .ok_or(MaskFormError::MissingPresentationInput)?;
        let interaction = required_port(
            form.front.outputs(),
            "interaction",
            PRESENTATION_INTERACTION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )
        .ok_or(MaskFormError::MissingInteractionOutput)?;
        let show = required_port(
            form.front.outputs(),
            "show",
            MANIFESTATION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Value,
        )
        .ok_or(MaskFormError::MissingShowOutput)?;
        Ok(Self {
            form_identity: FormIdentity {
                source_document_id: form.expanded.source_document_id.clone(),
                checked_form_id: form.expanded.checked_form_id.clone(),
                expanded_form_id: form.expanded.expanded_form_id.clone(),
            },
            form_name: form.expanded.name.clone(),
            presentation_input: boundary(&form.input_bindings, &presentation.port_id)
                .ok_or(MaskFormError::MissingPresentationInput)?,
            interaction_output: boundary(&form.output_bindings, &interaction.port_id)
                .ok_or(MaskFormError::MissingInteractionOutput)?,
            show_output: boundary(&form.output_bindings, &show.port_id)
                .ok_or(MaskFormError::MissingShowOutput)?,
        })
    }
}

impl PlannedMaskForm {
    pub fn admit(mask: &MaskForm, plan: &Plan) -> Result<Self, MaskFormError> {
        if !conduit_core::verify_plan(plan)
            || plan.source_document_id != mask.form_identity.source_document_id
            || plan.checked_form_id != mask.form_identity.checked_form_id
            || plan.expanded_form_id != mask.form_identity.expanded_form_id
        {
            return Err(MaskFormError::StalePlan);
        }
        for boundary in [
            &mask.presentation_input,
            &mask.interaction_output,
            &mask.show_output,
        ] {
            if !plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .any(|placement| placement.gear_id == boundary.gear_id)
            {
                return Err(MaskFormError::MissingPlannedBoundary);
            }
        }
        Ok(Self {
            mask: mask.clone(),
            plan: plan.clone(),
        })
    }

    pub fn show_placement(&self) -> &conduit_core::PlannedGear {
        self.plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|placement| placement.gear_id == self.mask.show_output.gear_id)
            .expect("admitted Mask Form retains its planned Show boundary")
    }
}

fn required_port<'a>(
    ports: &'a [PortDescriptor],
    name: &str,
    value_kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> Option<&'a PortDescriptor> {
    ports.iter().find(|port| {
        port.port_id.as_str() == name
            && port.value_kind.as_str() == value_kind
            && port.direction == direction
            && port.temporal == temporal
    })
}

#[cfg(feature = "form-catalog")]
fn boundary(
    bindings: &[conduit_form::AuthoringFrontBinding],
    port_id: &PortId,
) -> Option<MaskFormBoundary> {
    bindings
        .iter()
        .find(|binding| &binding.front_port_id == port_id)
        .map(|binding| MaskFormBoundary {
            front_port_id: binding.front_port_id.clone(),
            gear_id: binding.gear_id.clone(),
            gear_port_id: binding.gear_port_id.clone(),
        })
}
