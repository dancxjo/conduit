//! An ordinary Plot admitted to serve as a Body's presentation Mask.
//!
//! Mask is a role, not a second graph language. The checked and expanded Plot
//! own topology, typed Cords, finite bounds, cancellation, and terminal
//! semantics. This module only validates the narrow Face/interaction/Show
//! boundary and retains its exact ordinary Plot identity.

use alloc::string::String;
#[cfg(feature = "plot-catalog")]
use conduit_core::PortDescriptor;
use conduit_core::{GearId, Plan, PlotIdentity, PortDirection, PortId, PortTemporal};
use serde::{Deserialize, Serialize};

use crate::{FACE_INTERACTION_VALUE_KIND, PRESENTATION_VALUE_KIND, SHOW_VALUE_KIND};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskPlot {
    pub plot_identity: PlotIdentity,
    pub plot_name: String,
    pub face_input: MaskPlotBoundary,
    pub interaction_output: MaskPlotBoundary,
    pub show_output: MaskPlotBoundary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaskPlotBoundary {
    pub front_port_id: PortId,
    pub gear_id: GearId,
    pub gear_port_id: PortId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlannedMaskPlot {
    pub mask: MaskPlot,
    /// The ordinary immutable Plan for this exact expanded Mask Plot.
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskPlotError {
    InvalidPlot,
    InvalidFront,
    MissingFaceInput,
    MissingInteractionOutput,
    MissingShowOutput,
    StalePlan,
    MissingPlannedBoundary,
}

impl core::fmt::Display for MaskPlotError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid Mask Plot: {self:?}")
    }
}

#[cfg(feature = "plot-catalog")]
impl MaskPlot {
    /// Admit one canonical authoring Plot as a Mask. The graph remains an
    /// ordinary Plot graph; only its public role boundary is Mask-specific.
    pub fn admit(plot: &conduit_plot::ExpandedAuthoringPlot) -> Result<Self, MaskPlotError> {
        plot.expanded
            .validate_expansion()
            .map_err(|_| MaskPlotError::InvalidPlot)?;
        if plot.front.inputs().len() != 1
            || plot.front.outputs().len() != 2
            || plot.input_bindings.len() != 1
            || plot.output_bindings.len() != 2
        {
            return Err(MaskPlotError::InvalidFront);
        }
        let face = required_port(
            plot.front.inputs(),
            "face",
            PRESENTATION_VALUE_KIND,
            PortDirection::Input,
            PortTemporal::Value,
        )
        .ok_or(MaskPlotError::MissingFaceInput)?;
        let interaction = required_port(
            plot.front.outputs(),
            "interaction",
            FACE_INTERACTION_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Flow { closes: true },
        )
        .ok_or(MaskPlotError::MissingInteractionOutput)?;
        let show = required_port(
            plot.front.outputs(),
            "show",
            SHOW_VALUE_KIND,
            PortDirection::Output,
            PortTemporal::Value,
        )
        .ok_or(MaskPlotError::MissingShowOutput)?;
        Ok(Self {
            plot_identity: PlotIdentity {
                source_document_id: plot.expanded.source_document_id.clone(),
                checked_plot_id: plot.expanded.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded.expanded_plot_id.clone(),
            },
            plot_name: plot.expanded.name.clone(),
            face_input: boundary(&plot.input_bindings, &face.port_id)
                .ok_or(MaskPlotError::MissingFaceInput)?,
            interaction_output: boundary(&plot.output_bindings, &interaction.port_id)
                .ok_or(MaskPlotError::MissingInteractionOutput)?,
            show_output: boundary(&plot.output_bindings, &show.port_id)
                .ok_or(MaskPlotError::MissingShowOutput)?,
        })
    }
}

impl PlannedMaskPlot {
    pub fn admit(mask: &MaskPlot, plan: &Plan) -> Result<Self, MaskPlotError> {
        if !conduit_core::verify_plan(plan)
            || plan.source_document_id != mask.plot_identity.source_document_id
            || plan.checked_plot_id != mask.plot_identity.checked_plot_id
            || plan.expanded_plot_id != mask.plot_identity.expanded_plot_id
        {
            return Err(MaskPlotError::StalePlan);
        }
        for (boundary, direction, value_kind, temporal) in [
            (
                &mask.face_input,
                PortDirection::Input,
                PRESENTATION_VALUE_KIND,
                PortTemporal::Value,
            ),
            (
                &mask.interaction_output,
                PortDirection::Output,
                FACE_INTERACTION_VALUE_KIND,
                PortTemporal::Flow { closes: true },
            ),
            (
                &mask.show_output,
                PortDirection::Output,
                SHOW_VALUE_KIND,
                PortTemporal::Value,
            ),
        ] {
            if !plan.fragments.iter().any(|fragment| {
                let Some(placement) = fragment
                    .placements
                    .iter()
                    .find(|placement| placement.gear_id == boundary.gear_id)
                else {
                    return false;
                };
                fragment.fore_ports.iter().any(|port| {
                    port.front_port_id == boundary.front_port_id
                        && port.direction == direction
                        && port.placement_id == placement.placement_id
                        && port.gear_port_id == boundary.gear_port_id
                        && port.value_kind.as_str() == value_kind
                        && port.temporal == temporal
                })
            }) {
                return Err(MaskPlotError::MissingPlannedBoundary);
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
            .expect("admitted Mask Plot retains its planned Show boundary")
    }
}

#[cfg(feature = "plot-catalog")]
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

#[cfg(feature = "plot-catalog")]
fn boundary(
    bindings: &[conduit_plot::AuthoringFrontBinding],
    port_id: &PortId,
) -> Option<MaskPlotBoundary> {
    bindings
        .iter()
        .find(|binding| &binding.front_port_id == port_id)
        .map(|binding| MaskPlotBoundary {
            front_port_id: binding.front_port_id.clone(),
            gear_id: binding.gear_id.clone(),
            gear_port_id: binding.gear_port_id.clone(),
        })
}
