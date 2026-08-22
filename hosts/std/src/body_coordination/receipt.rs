use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoordinationRole {
    Forebrain,
    Motherbrain,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinationFailure {
    PeerAbsent,
    WrongBoot,
    Malformed,
    Oversized,
    Pressure,
    Duplicate,
    LossBeforeAcceptance,
    LossAfterAcceptance,
    TerminalDisagreement,
    Cancelled,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct BodyCoordinationReceipt {
    pub schema: String,
    pub role: CoordinationRole,
    pub body_id: String,
    pub part_id: String,
    pub peer_part_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub peer_host_id: String,
    pub peer_boot_id: String,
    pub plan_id: String,
    pub fragment_id: String,
    pub active_play_id: String,
    pub outbound_cord_id: u16,
    pub inbound_cord_id: u16,
    pub outbound_line_id: String,
    pub inbound_line_id: String,
    pub base_instance_id: String,
    pub offered: bool,
    pub accepted: bool,
    pub delivered: bool,
    pub input_closed: bool,
    pub terminal: String,
    pub received: String,
    pub authority: String,
}

impl BodyCoordinationReceipt {
    pub const SCHEMA: &'static str = "conduit.pete/body-coordination@1";

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != Self::SCHEMA
            || self.body_id.is_empty()
            || self.part_id.is_empty()
            || self.peer_part_id.is_empty()
            || self.part_id == self.peer_part_id
            || self.host_id.is_empty()
            || self.boot_id.is_empty()
            || self.peer_host_id.is_empty()
            || self.peer_boot_id.is_empty()
            || self.plan_id.is_empty()
            || self.fragment_id.is_empty()
            || self.active_play_id.is_empty()
            || self.outbound_line_id.is_empty()
            || self.inbound_line_id.is_empty()
            || self.outbound_line_id == self.inbound_line_id
            || self.base_instance_id.is_empty()
            || !self.offered
            || !self.accepted
            || !self.delivered
            || !self.input_closed
            || self.terminal != "completed"
            || self.received.is_empty()
            || self.authority != "none"
        {
            return Err("coordination receipt is incomplete or invents authority".into());
        }
        Ok(())
    }
}
