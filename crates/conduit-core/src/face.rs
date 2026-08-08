use crate::{CapabilityOffer, PortDescriptor, PortId};
use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FaceStartupParameter {
    pub name: String,
    pub value_type: String,
    pub has_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckedFace {
    startup_parameters: Vec<FaceStartupParameter>,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    shorthand: Option<(PortId, PortId)>,
}

impl CheckedFace {
    pub fn new(
        startup_parameters: Vec<FaceStartupParameter>,
        mut inputs: Vec<PortDescriptor>,
        mut outputs: Vec<PortDescriptor>,
        shorthand: Option<(PortId, PortId)>,
    ) -> Self {
        inputs.sort_by(|left, right| left.port_id.as_str().cmp(right.port_id.as_str()));
        outputs.sort_by(|left, right| left.port_id.as_str().cmp(right.port_id.as_str()));
        Self {
            startup_parameters,
            inputs,
            outputs,
            shorthand,
        }
    }
}

impl CapabilityOffer {
    pub fn checked_face(&self) -> CheckedFace {
        let shorthand = self.shorthand.clone().or_else(|| {
            if let ([input], [output]) = (self.inputs.as_slice(), self.outputs.as_slice()) {
                Some((input.port_id.clone(), output.port_id.clone()))
            } else {
                None
            }
        });
        CheckedFace::new(
            self.startup_parameters.clone(),
            self.inputs.clone(),
            self.outputs.clone(),
            shorthand,
        )
    }
}
