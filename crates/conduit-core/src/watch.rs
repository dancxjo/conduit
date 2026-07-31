//! Exact-plan admission facts for non-interfering live-value observation.
//!
//! A Watch is an instrument control, not a semantic graph edge, recorder, or
//! source edit. This module only describes its finite envelope; hosted
//! runtimes own attached previews and must keep them outside cord pressure.

use crate::{Direction, Id, InstancePath, PinnedDescriptor, Sensitivity};

pub const WATCH_ADMISSION_SCHEMA_VERSION: u32 = 0;

/// An exact dataflow location an admitted Watch may observe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchSubject<'a> {
    Cord(Id<'a>),
    NodePort {
        node: InstancePath<'a>,
        port: Id<'a>,
        direction: Direction,
    },
}

/// Isolated retention selected by the plan. None of these modes can delay or
/// reject an observed cord offer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchRetention {
    Latest,
    Ring,
    Sample,
}

impl WatchRetention {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Latest => "latest",
            Self::Ring => "ring",
            Self::Sample => "sample",
        }
    }
}

/// One finite selectable Watch slot admitted by an exact execution plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchAdmission<'a> {
    pub id: Id<'a>,
    pub subject: WatchSubject<'a>,
    /// Exact value representation the host may use for previews.
    pub representation: PinnedDescriptor<'a>,
    /// Maximum material copied into one preview, excluding metadata.
    pub maximum_preview_bytes: u32,
    /// Maximum previews retained by an attached Watch.
    pub maximum_history: u16,
    /// Minimum source ticks between accepted previews. `Sample` uses this as
    /// its sampling period; all modes use it as an upper-rate bound.
    pub minimum_tick_interval: u64,
    pub retention: WatchRetention,
    /// The most-sensitive material this slot can structurally describe.
    pub sensitivity_ceiling: Sensitivity,
    /// Optional exact effect action required before non-public material is
    /// revealed. Absence still permits a redacted structural observation.
    pub reveal_action: Option<Id<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchAdmissionReason {
    UnsupportedVersion,
    EmptySlots,
    InvalidIdentity,
    DuplicateIdentity,
    InvalidBound,
    InvalidRetention,
    RevealActionRequired,
}

impl WatchAdmissionReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-WAT-001",
            Self::EmptySlots | Self::InvalidIdentity | Self::DuplicateIdentity => "CND-WAT-002",
            Self::InvalidBound | Self::InvalidRetention => "CND-WAT-003",
            Self::RevealActionRequired => "CND-WAT-004",
        }
    }
}

/// Validates a finite plan-owned Watch admission table without installing a
/// watch or accessing a value provider.
pub fn validate_watch_admissions(
    schema_version: u32,
    slots: &[WatchAdmission<'_>],
) -> Result<(), WatchAdmissionReason> {
    if schema_version != WATCH_ADMISSION_SCHEMA_VERSION {
        return Err(WatchAdmissionReason::UnsupportedVersion);
    }
    if slots.is_empty() {
        return Err(WatchAdmissionReason::EmptySlots);
    }
    for (index, slot) in slots.iter().enumerate() {
        if slot.id.as_str().is_empty()
            || slot.maximum_preview_bytes == 0
            || slot.maximum_history == 0
            || slot.minimum_tick_interval == 0
        {
            return Err(WatchAdmissionReason::InvalidBound);
        }
        if slots[..index].iter().any(|prior| prior.id == slot.id) {
            return Err(WatchAdmissionReason::DuplicateIdentity);
        }
        if matches!(slot.retention, WatchRetention::Latest) && slot.maximum_history != 1 {
            return Err(WatchAdmissionReason::InvalidRetention);
        }
        if slot.sensitivity_ceiling != Sensitivity::Public && slot.reveal_action.is_none() {
            return Err(WatchAdmissionReason::RevealActionRequired);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SemanticHash;

    const SLOT: WatchAdmission<'static> = WatchAdmission {
        id: Id("watch/output"),
        subject: WatchSubject::Cord(Id("cord/output")),
        representation: PinnedDescriptor {
            id: Id("representation/utf8"),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes([4; 32]),
        },
        maximum_preview_bytes: 32,
        maximum_history: 1,
        minimum_tick_interval: 1,
        retention: WatchRetention::Latest,
        sensitivity_ceiling: Sensitivity::Public,
        reveal_action: None,
    };

    #[test]
    fn admissions_are_finite_and_reveal_nonpublic_material_only_explicitly() {
        assert!(validate_watch_admissions(0, &[SLOT]).is_ok());
        assert_eq!(
            validate_watch_admissions(0, &[]),
            Err(WatchAdmissionReason::EmptySlots)
        );
        let mut ring = SLOT;
        ring.retention = WatchRetention::Ring;
        ring.maximum_history = 2;
        ring.sensitivity_ceiling = Sensitivity::Restricted;
        assert_eq!(
            validate_watch_admissions(0, &[ring]),
            Err(WatchAdmissionReason::RevealActionRequired)
        );
        ring.reveal_action = Some(Id("conduit/data.present"));
        assert!(validate_watch_admissions(0, &[ring]).is_ok());
    }
}
