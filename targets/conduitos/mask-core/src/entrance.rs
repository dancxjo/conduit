//! Finite observational entrance facts. No lifecycle action or hardware authority.
mod scene;
use crate::{
    control,
    data::{self, Refusal},
};
use alloc::{string::String, vec::Vec};
pub use scene::{current_detail, scene};
use serde::{Deserialize, Serialize};
pub const MAX_FACT_BYTES: usize = 8192;
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub control: control::Scope,
    pub outer_revision: u64,
    pub journey_revision: u64,
    pub width: u16,
    pub height: u16,
}
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum ConnectivityStatus {
    Current,
    PeerAttached,
    ValueVisible,
    Lost,
}
impl ConnectivityStatus {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Current => "USB LINE CURRENT",
            Self::PeerAttached => "PEER ATTACHED / MEMBERSHIP NOT REQUESTED",
            Self::ValueVisible => "LINE VALUE VISIBLE",
            Self::Lost => "USB LINE LOST / STALE SESSION REFUSED",
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Connectivity {
    pub body: String,
    pub line: String,
    pub status: ConnectivityStatus,
    pub value: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BirthAction {
    pub binding: String,
    pub name: String,
    pub unavailable: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub scope: Scope,
    pub profile: String,
    pub build: String,
    pub image: String,
    pub offer_count: u64,
    pub offer_generation: u64,
    pub plot: String,
    pub source: String,
    pub checked: String,
    /// Expanded Plot, Body, Wake, Plan, Play, input Sign, result Sign, in that order.
    pub lifecycle: [Option<String>; 7],
    pub connectivity: Option<Connectivity>,
    pub birth: Option<BirthAction>,
}
impl Facts {
    pub fn validate(&self) -> Result<(), Refusal> {
        if self.scope.outer_revision == 0 {
            return Err(Refusal::Bounds);
        }
        let mut total = 0usize;
        let mut check = |s: &str| {
            total = total.checked_add(s.len()).ok_or(Refusal::Bounds)?;
            if s.len() > 2048 || total > MAX_FACT_BYTES {
                return Err(Refusal::Bounds);
            }
            Ok(())
        };
        for s in [
            self.scope.control.host.as_str(),
            self.scope.control.boot.as_str(),
            &self.profile,
            &self.build,
            &self.image,
            &self.plot,
            &self.source,
            &self.checked,
        ] {
            check(s)?;
        }
        for s in self.lifecycle.iter().flatten() {
            check(s)?;
        }
        if let Some(line) = &self.connectivity {
            check(&line.body)?;
            check(&line.line)?;
            if let Some(v) = &line.value {
                check(v)?;
            }
        }
        if let Some(birth) = &self.birth {
            check(&birth.binding)?;
            check(&birth.name)?;
            if let Some(v) = &birth.unavailable {
                check(v)?;
            }
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<[u8; 32], Refusal> {
        Ok(conduit_core::semantic_digest(
            "conduitos/mask-entrance-facts@1",
            &data::encode(self)?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Detail {
    Profile,
    Build,
    Image,
    Host,
    Boot,
    Offers,
    Plot,
    Source,
    Checked,
    Expanded,
    Body,
    Wake,
    Plan,
    Play,
    Input,
    Result,
}
impl Detail {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Profile => "PROFILE ID",
            Self::Build => "BUILD ID",
            Self::Image => "IMAGE BINDING",
            Self::Host => "HOST ID",
            Self::Boot => "BOOT ID",
            Self::Offers => "CURRENT OFFERS",
            Self::Plot => "PLOT SUBJECT",
            Self::Source => "SOURCE DOCUMENT ID",
            Self::Checked => "CHECKED PLOT ID",
            Self::Expanded => "EXPANDED PLOT ID",
            Self::Body => "BODY ID",
            Self::Wake => "WAKE ID",
            Self::Plan => "PLAN ID",
            Self::Play => "ACTIVE PLAY ID",
            Self::Input => "INPUT SIGN ID",
            Self::Result => "RESULT SIGN ID",
        }
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub scope: Scope,
    pub control_revision: u64,
    pub digest: [u8; 32],
    pub workspace: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DetailReceipt {
    pub scope: Scope,
    pub control_revision: u64,
    pub digest: [u8; 32],
    pub kind: Detail,
    pub value: String,
}

pub fn encode_scene(
    receipt: &Receipt,
    scene: conduit_face::GraphicsScene,
) -> Result<Vec<u8>, Refusal> {
    data::encode_scene(receipt, scene)
}
pub fn decode_scene(bytes: &[u8]) -> Result<(Receipt, conduit_face::GraphicsScene), Refusal> {
    data::decode_scene(bytes)
}
