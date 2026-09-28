//! Portable kind contracts and their finite configuration/terminal behavior.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    CapabilityLimits, FrontValueBound, FrontValueLocation, Kind, KindConfigurationField, KindId,
    KindSemanticLaw, KindTerminalBehavior, PortDescriptor, PortDirection,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardKindContract {
    pub kind_id: KindId,
    pub plain_name: String,
    pub summary: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<KindConfigurationField>,
    pub limits: CapabilityLimits,
    pub terminal_behavior: KindTerminalBehavior,
    pub hosted_implementation_required: bool,
    pub browser_manifestation_honest: bool,
    pub pico_manifestation_honest: bool,
    pub example: String,
}

impl StandardKindContract {
    pub fn into_semantic_contract(self, revision: &str) -> Kind {
        let value_bounds = self
            .inputs
            .iter()
            .chain(&self.outputs)
            .filter_map(|port| {
                portable_value_maximum_bytes(port.value_kind.as_str()).map(|maximum_bytes| {
                    FrontValueBound {
                        location: match port.direction {
                            PortDirection::Input => FrontValueLocation::Input(port.port_id.clone()),
                            PortDirection::Output => {
                                FrontValueLocation::Output(port.port_id.clone())
                            }
                        },
                        maximum_bytes,
                    }
                })
            })
            .collect::<Vec<_>>();
        let mut semantic_laws = alloc::vec![KindSemanticLaw::Terminal(self.terminal_behavior)];
        if !value_bounds.is_empty() {
            semantic_laws.push(KindSemanticLaw::ValueBounds(value_bounds));
        }
        Kind {
            startup_parameters: crate::startup_front(&self.configuration),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: revision.into(),
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: self.configuration,
            semantic_laws,
            limits: self.limits,
        }
    }
}

/// Domain-owned finite envelopes for the portable Face values whose encoding
/// size is not implied by a primitive core Info Kind.
fn portable_value_maximum_bytes(kind: &str) -> Option<u64> {
    match kind {
        conduit_presentation::PRESENTATION_COMPOSITION_KIND => {
            Some(conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES as u64)
        }
        conduit_presentation::LAYOUT_FRAME_KIND => {
            Some(conduit_presentation::MAX_LAYOUT_FRAME_BYTES as u64)
        }
        conduit_presentation::GRAPHICS_SCENE_KIND => {
            Some(conduit_presentation::MAX_GRAPHICS_SCENE_BYTES as u64)
        }
        conduit_presentation::BITMAP_PRESENTATION_KIND => {
            Some(conduit_presentation::MAX_GRAY8_BITMAP_BYTES as u64)
        }
        _ => None,
    }
}
