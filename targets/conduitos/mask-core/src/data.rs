//! Pointer-free observational facts and advisory selection receipts.
use crate::{MAX_BLOB, MAX_VIEW_BYTES};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum Refusal {
    Bounds,
    Malformed,
    Order,
    Capacity,
    Layout,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum PresentationStatus {
    World,
    PlotOpened,
    BornLulled,
    Awake,
    Planned,
    QuiescentAwaitingInput,
    SemanticCompleted,
    Lulled,
    Fulfilled,
    InputUnavailable,
    Stopped,
}
impl PresentationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::World => "world",
            Self::PlotOpened => "plot-opened",
            Self::BornLulled => "born-lulled",
            Self::Awake => "awake",
            Self::Planned => "planned",
            Self::QuiescentAwaitingInput => "quiescent-awaiting-input",
            Self::SemanticCompleted => "semantic-completed",
            Self::Lulled => "lulled",
            Self::Fulfilled => "fulfilled",
            Self::InputUnavailable => "input-unavailable",
            Self::Stopped => "stopped",
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub session: u64,
    pub host: conduit_core::HostId,
    pub boot: conduit_core::BootId,
    pub body: Option<conduit_body::BodyId>,
    pub journey_revision: u64,
    pub outer_revision: u64,
    pub view_digest: [u8; 32],
    pub foreground: conduit_body::ResidentPlot,
    pub width: u16,
    pub height: u16,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Notice {
    pub heading: String,
    pub reason: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub friendly_name: Option<String>,
    pub foreground_title: String,
    pub status: PresentationStatus,
    pub loss_recovery: Option<String>,
    pub result: Option<String>,
    pub result_omitted_bytes: u64,
    pub patchbay_graph_available: bool,
    pub refusal: Option<Notice>,
    pub details_open: bool,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Update {
    pub scope: Scope,
    pub facts: Facts,
    pub view: Option<Vec<u8>>,
}
impl Update {
    pub fn validate(&self) -> Result<(), Refusal> {
        let s = &self.scope;
        if s.session == 0
            || s.outer_revision == 0
            || s.width == 0
            || s.height == 0
            || s.view_digest != view_digest(self.view.as_deref())
        {
            return Err(Refusal::Bounds);
        }
        for value in [
            Some(s.host.as_str()),
            Some(s.boot.as_str()),
            s.body.as_ref().map(|id| id.as_str()),
            Some(s.foreground.source_document_id.as_str()),
            Some(s.foreground.checked_plot_id.as_str()),
        ]
        .into_iter()
        .flatten()
        {
            if value.is_empty() || value.len() > 2048 || value.chars().any(char::is_control) {
                return Err(Refusal::Bounds);
            }
        }
        if self
            .view
            .as_ref()
            .is_some_and(|bytes| bytes.is_empty() || bytes.len() > MAX_VIEW_BYTES)
        {
            return Err(Refusal::Bounds);
        }
        for value in [
            self.facts.friendly_name.as_deref(),
            Some(self.facts.foreground_title.as_str()),
            self.facts.loss_recovery.as_deref(),
            self.facts.result.as_deref(),
            self.facts.refusal.as_ref().map(|n| n.heading.as_str()),
            self.facts.refusal.as_ref().map(|n| n.reason.as_str()),
        ]
        .into_iter()
        .flatten()
        {
            if value.len() > 2048 {
                return Err(Refusal::Bounds);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub scope: Scope,
    pub reader_revision: u64,
    pub page: u32,
    pub pages: u32,
    pub selected_node: Option<u16>,
}
#[derive(Serialize, Deserialize)]
pub enum Command {
    Control {
        expected: u64,
        scope: crate::control::Scope,
        operation: crate::control::Operation,
    },
    Update {
        expected: u64,
        update: Update,
    },
    Navigate {
        expected: u64,
        key: [u8; 3],
    },
    Reset {
        expected: u64,
    },
    Frame {
        expected: u64,
    },
    Selection {
        expected: u64,
    },
}
#[derive(Serialize, Deserialize)]
pub enum Reply {
    Control {
        changed: bool,
        snapshot: crate::control::Snapshot,
    },
    Updated {
        snapshot: Snapshot,
    },
    Navigated {
        changed: bool,
        snapshot: Snapshot,
    },
    Selected {
        snapshot: Snapshot,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FrameReceipt {
    pub snapshot: Snapshot,
}
pub fn view_digest(view: Option<&[u8]>) -> [u8; 32] {
    match view {
        Some(bytes) => conduit_core::semantic_digest("conduitos/workspace-mask-view/some@1", bytes),
        None => conduit_core::semantic_digest("conduitos/workspace-mask-view/none@1", &[]),
    }
}
pub fn encode<T: Serialize>(value: &T) -> Result<Vec<u8>, Refusal> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(MAX_BLOB)
        .map_err(|_| Refusal::Capacity)?;
    bytes.resize(MAX_BLOB, 0);
    let n = serde_json_core::to_slice(value, &mut bytes).map_err(|_| Refusal::Capacity)?;
    bytes.truncate(n);
    Ok(bytes)
}
pub fn decode<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T, Refusal> {
    if bytes.is_empty() || bytes.len() > MAX_BLOB {
        return Err(Refusal::Bounds);
    }
    serde_json::from_slice(bytes).map_err(|_| Refusal::Malformed)
}
pub fn encode_frame(
    receipt: &FrameReceipt,
    scene: conduit_face::GraphicsScene,
) -> Result<Vec<u8>, Refusal> {
    let metadata = encode(receipt)?;
    let length = scene.encoded_len();
    let total = 4usize
        .checked_add(metadata.len())
        .and_then(|n| n.checked_add(length))
        .filter(|n| *n <= MAX_BLOB)
        .ok_or(Refusal::Capacity)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| Refusal::Capacity)?;
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&metadata);
    bytes.extend_from_slice(&scene.encode()[..length]);
    Ok(bytes)
}
pub fn decode_frame(bytes: &[u8]) -> Result<(FrameReceipt, conduit_face::GraphicsScene), Refusal> {
    if bytes.len() > MAX_BLOB {
        return Err(Refusal::Bounds);
    }
    let size = u32::from_le_bytes(
        bytes
            .get(..4)
            .ok_or(Refusal::Malformed)?
            .try_into()
            .map_err(|_| Refusal::Malformed)?,
    ) as usize;
    let end = 4usize
        .checked_add(size)
        .filter(|n| *n <= bytes.len())
        .ok_or(Refusal::Bounds)?;
    Ok((
        decode(&bytes[4..end])?,
        conduit_face::GraphicsScene::decode(&bytes[end..]).map_err(|_| Refusal::Malformed)?,
    ))
}
