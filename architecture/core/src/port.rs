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
    /// Exact bounded semantic info carried by this endpoint's `!` track.
    /// Absence means the Fore does not promise an abnormal terminal track.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abnormal_kind: Option<KindId>,
}

impl PortDescriptor {
    /// Match only the payload selected by a Fore binding. An output may expose
    /// payload without promising its sibling abnormal track; unhandled abnormal
    /// truth remains a separate Plot obligation. An explicitly promised abnormal
    /// kind must match exactly. Input contracts retain exact channel matching.
    pub fn matches_fore_payload(
        &self,
        direction: PortDirection,
        value_kind: &KindId,
        temporal: PortTemporal,
        abnormal_kind: Option<&KindId>,
    ) -> bool {
        self.direction == direction
            && &self.value_kind == value_kind
            && self.temporal == temporal
            && (self.abnormal_kind.as_ref() == abnormal_kind
                || (direction == PortDirection::Output && abnormal_kind.is_none()))
    }
}
