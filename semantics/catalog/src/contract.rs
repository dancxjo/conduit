//! Portable kind contracts and their finite configuration/terminal behavior.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    CapabilityLimits, CheckedValueContract, FrontValueContract, FrontValueLocation, Kind,
    KindConfigurationField, KindId, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection,
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
        let value_contracts = self
            .inputs
            .iter()
            .chain(&self.outputs)
            .filter_map(|port| {
                portable_value_bounds(self.kind_id.as_str(), port.value_kind.as_str()).map(
                    |(minimum_bytes, maximum_bytes)| FrontValueContract {
                        location: match port.direction {
                            PortDirection::Input => FrontValueLocation::Input(port.port_id.clone()),
                            PortDirection::Output => {
                                FrontValueLocation::Output(port.port_id.clone())
                            }
                        },
                        contract: CheckedValueContract::new(
                            port.value_kind.clone(),
                            maximum_bytes as u32,
                            if minimum_bytes == 0 {
                                Vec::new()
                            } else {
                                alloc::vec![conduit_core::ValueConstraint::ByteLength {
                                    minimum: minimum_bytes as u32,
                                    maximum: maximum_bytes as u32,
                                }]
                            },
                        )
                        .expect("portable Face value envelope is finite"),
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut semantic_laws = alloc::vec![KindSemanticLaw::Terminal(self.terminal_behavior)];
        if !value_contracts.is_empty() {
            semantic_laws.push(KindSemanticLaw::ValueContracts(value_contracts));
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
fn portable_value_bounds(owner: &str, kind: &str) -> Option<(u64, u64)> {
    // Only these existing Fores promise the canonical decimal capsule. Other
    // Quantity realization profiles keep their own admitted representation bounds.
    if kind == conduit_core::QUANTITY_INFO_ID
        && matches!(
            owner,
            crate::QUANTITY_MAP_KIND | crate::QUANTITY_INFO_WRAP_KIND
        )
    {
        let bytes = conduit_core::QUANTITY_ENCODED_LEN as u64;
        return Some((bytes, bytes));
    }
    if owner == crate::QUANTITY_INFO_WRAP_KIND
        && kind
            == crate::wrapped_quantity_type()
                .profile()
                .expect("canonical Quantity profile")
                .value_kind()
                .as_str()
    {
        let bytes =
            (crate::quantity_info_prefix().len() + conduit_core::QUANTITY_ENCODED_LEN) as u64;
        return Some((bytes, bytes));
    }
    match kind {
        conduit_face::FACE_COMPOSITION_KIND => {
            Some((0, conduit_face::MAX_FACE_COMPOSITION_BYTES as u64))
        }
        conduit_face::LAYOUT_FRAME_KIND => Some((0, conduit_face::MAX_LAYOUT_FRAME_BYTES as u64)),
        conduit_face::GRAPHICS_SCENE_KIND => {
            Some((0, conduit_face::MAX_GRAPHICS_SCENE_BYTES as u64))
        }
        conduit_face::BITMAP_FACE_KIND => Some((0, conduit_face::MAX_GRAY8_BITMAP_BYTES as u64)),
        _ => None,
    }
}
