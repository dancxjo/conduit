use alloc::string::String;
use serde::{Deserialize, Serialize};

pub const MAX_PATCHBAY_CONTROL_ID_BYTES: usize = 768;

/// Portable semantic actions shared by every Patchbay renderer and Host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchbayAction {
    OpenBack,
    Save,
    ToggleLinearView,
    Birth,
    Wake,
    Lull,
    Plan,
    Play,
    Stop,
    Hold,
    PlaceGear,
    DuplicateGear,
    RemoveGear,
    RemoveCord,
    ConnectPorts,
    RerouteCord,
    ConfigureGear,
}

impl PatchbayAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenBack => "open-back",
            Self::Save => "save",
            Self::ToggleLinearView => "toggle-linear-view",
            Self::Birth => "birth",
            Self::Wake => "wake",
            Self::Lull => "lull",
            Self::Plan => "plan",
            Self::Play => "play",
            Self::Stop => "stop",
            Self::Hold => "hold",
            Self::PlaceGear => "place-gear",
            Self::DuplicateGear => "duplicate-gear",
            Self::RemoveGear => "remove-gear",
            Self::RemoveCord => "remove-cord",
            Self::ConnectPorts => "connect-ports",
            Self::RerouteCord => "reroute-cord",
            Self::ConfigureGear => "configure-gear",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        Some(match value {
            "open-back" => Self::OpenBack,
            "save" => Self::Save,
            "toggle-linear-view" => Self::ToggleLinearView,
            "birth" => Self::Birth,
            "wake" => Self::Wake,
            "lull" => Self::Lull,
            "plan" => Self::Plan,
            "play" => Self::Play,
            "stop" => Self::Stop,
            "hold" => Self::Hold,
            "place-gear" => Self::PlaceGear,
            "duplicate-gear" => Self::DuplicateGear,
            "remove-gear" => Self::RemoveGear,
            "remove-cord" => Self::RemoveCord,
            "connect-ports" => Self::ConnectPorts,
            "reroute-cord" => Self::RerouteCord,
            "configure-gear" => Self::ConfigureGear,
            _ => return None,
        })
    }

    pub const fn presentation_intent(self) -> &'static str {
        match self {
            Self::OpenBack => "conduit.intent/open@1",
            Self::Save => "conduit.intent/save@1",
            Self::ToggleLinearView => "conduit.intent/toggle-linear-view@1",
            Self::Birth => "conduit.intent/birth@1",
            Self::Wake => "conduit.intent/wake@1",
            Self::Lull => "conduit.intent/lull@1",
            Self::Plan => "conduit.intent/plan@1",
            Self::Play => "conduit.intent/play@1",
            Self::Stop => "conduit.intent/stop@1",
            Self::Hold => "conduit.intent/hold@1",
            Self::PlaceGear => "conduit.intent/place-gear@1",
            Self::DuplicateGear => "conduit.intent/duplicate-gear@1",
            Self::RemoveGear => "conduit.intent/remove-gear@1",
            Self::RemoveCord => "conduit.intent/remove-cord@1",
            Self::ConnectPorts => "conduit.intent/connect-ports@1",
            Self::RerouteCord => "conduit.intent/reroute-cord@1",
            Self::ConfigureGear => "conduit.intent/configure-gear@1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayControlError {
    EmptyIdentity,
    IdentityTooLong,
}

/// Typed renderer-to-semantic-control request. Geometry and key codes never
/// cross this seam.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchbayControlRequest {
    pub request_id: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub action_id: String,
    pub action: PatchbayAction,
    pub target_identity: String,
}

impl PatchbayControlRequest {
    pub fn new(
        request_id: impl Into<String>,
        presentation_id: impl Into<String>,
        presentation_revision: u64,
        action_id: impl Into<String>,
        action: PatchbayAction,
        target_identity: impl Into<String>,
    ) -> Result<Self, PatchbayControlError> {
        let request_id = request_id.into();
        let presentation_id = presentation_id.into();
        let action_id = action_id.into();
        let target_identity = target_identity.into();
        for value in [&request_id, &presentation_id, &action_id, &target_identity] {
            if value.is_empty() {
                return Err(PatchbayControlError::EmptyIdentity);
            }
            if value.len() > MAX_PATCHBAY_CONTROL_ID_BYTES {
                return Err(PatchbayControlError::IdentityTooLong);
            }
        }
        Ok(Self {
            request_id,
            presentation_id,
            presentation_revision,
            action_id,
            action,
            target_identity,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_request_is_bounded_and_round_trips_every_action_name() {
        for action in [
            PatchbayAction::OpenBack,
            PatchbayAction::Birth,
            PatchbayAction::Wake,
            PatchbayAction::Plan,
            PatchbayAction::Play,
            PatchbayAction::Stop,
            PatchbayAction::Lull,
        ] {
            assert_eq!(PatchbayAction::from_name(action.as_str()), Some(action));
            assert!(PatchbayControlRequest::new(
                "request",
                "presentation",
                7,
                "action/current",
                action,
                "target"
            )
            .is_ok());
        }
        assert_eq!(PatchbayAction::from_name("be-born"), None);
        assert_eq!(PatchbayAction::from_name("be_born"), None);
        assert!(PatchbayControlRequest::new(
            "",
            "presentation",
            0,
            "action/current",
            PatchbayAction::Play,
            "target"
        )
        .is_err());
    }
}
