use crate::{KindId, PortId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortDirection {
    Input,
    Output,
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PortTemporal {
    #[default]
    Value,
    Flow {
        closes: bool,
    },
    Current,
}

impl PortTemporal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Value => "value",
            Self::Flow { closes: false } => "flow-open",
            Self::Flow { closes: true } => "flow-closing",
            Self::Current => "current",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortDescriptor {
    pub port_id: PortId,
    pub value_kind: KindId,
    pub direction: PortDirection,
    #[serde(default)]
    pub temporal: PortTemporal,
}
