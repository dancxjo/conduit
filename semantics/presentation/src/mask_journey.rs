//! The one ordered semantic Mask journey shared by every embodiment.
//!
//! This is an action contract, not an expected-output script. Each Body must
//! perform these actions against its own wardrobe, Hosts, Plans, and Shows and
//! retain its own outcome, including honest refusal.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MaskJourneyAction {
    InspectInitialShow,
    WearAlternateMask,
    PreferAlternateMask,
    WithdrawSelectedRoute,
    InspectUnavailableShow,
    AddFaceHost,
    AdmitReplacementPlan,
    InspectReplannedShow,
    DoffAlternateMask,
    InspectRestoredShow,
}

impl MaskJourneyAction {
    pub const fn id(self) -> &'static str {
        match self {
            Self::InspectInitialShow => "mask.inspect-initial-show",
            Self::WearAlternateMask => "mask.wear-alternate",
            Self::PreferAlternateMask => "mask.prefer-alternate",
            Self::WithdrawSelectedRoute => "mask.withdraw-selected-route",
            Self::InspectUnavailableShow => "mask.inspect-unavailable-show",
            Self::AddFaceHost => "mask.add-face-host",
            Self::AdmitReplacementPlan => "mask.admit-replacement-plan",
            Self::InspectReplannedShow => "mask.inspect-replanned-show",
            Self::DoffAlternateMask => "mask.doff-alternate",
            Self::InspectRestoredShow => "mask.inspect-restored-show",
        }
    }
}

pub const MASK_JOURNEY_ACTIONS: [MaskJourneyAction; 10] = [
    MaskJourneyAction::InspectInitialShow,
    MaskJourneyAction::WearAlternateMask,
    MaskJourneyAction::PreferAlternateMask,
    MaskJourneyAction::WithdrawSelectedRoute,
    MaskJourneyAction::InspectUnavailableShow,
    MaskJourneyAction::AddFaceHost,
    MaskJourneyAction::AdmitReplacementPlan,
    MaskJourneyAction::InspectReplannedShow,
    MaskJourneyAction::DoffAlternateMask,
    MaskJourneyAction::InspectRestoredShow,
];

/// One embodiment supplies material behavior for an action; it does not own
/// ordering or decide which actions belong to the Journey.
pub trait MaskJourneyEmbodiment {
    type Outcome;
    type Error;

    fn perform(&mut self, action: MaskJourneyAction) -> Result<Self::Outcome, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskJourneyRunError<E> {
    pub action: MaskJourneyAction,
    pub source: E,
}

/// Actualize the canonical Journey once through one embodiment.
///
/// `retain` receives producer-owned evidence only after the action succeeds.
/// A semantic refusal is therefore an ordinary successful outcome; transport,
/// harness, or execution failure belongs in `Error` and stops the Journey.
pub fn actualize_mask_journey<E, R>(
    embodiment: &mut E,
    mut retain: R,
) -> Result<(), MaskJourneyRunError<E::Error>>
where
    E: MaskJourneyEmbodiment,
    R: FnMut(MaskJourneyAction, &E::Outcome),
{
    let mut cursor = MaskJourneyCursor::new();
    for action in MASK_JOURNEY_ACTIONS {
        debug_assert_eq!(cursor.next_action(), Some(action));
        let outcome = embodiment
            .perform(action)
            .map_err(|source| MaskJourneyRunError { action, source })?;
        cursor
            .advance(action)
            .expect("canonical Mask Journey advances its own exact order");
        retain(action, &outcome);
    }
    debug_assert!(cursor.is_complete());
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MaskJourneyCursor {
    next: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskJourneyOrderError {
    OutOfOrder {
        expected: MaskJourneyAction,
        observed: MaskJourneyAction,
    },
    AlreadyComplete,
}

impl MaskJourneyCursor {
    pub const fn new() -> Self {
        Self { next: 0 }
    }

    pub fn next_action(&self) -> Option<MaskJourneyAction> {
        MASK_JOURNEY_ACTIONS.get(self.next).copied()
    }

    /// Retain that one embodiment actually completed the next semantic action.
    /// Outcome evidence remains embodiment-owned and is recorded separately.
    pub fn advance(&mut self, completed: MaskJourneyAction) -> Result<(), MaskJourneyOrderError> {
        let Some(expected) = self.next_action() else {
            return Err(MaskJourneyOrderError::AlreadyComplete);
        };
        if completed != expected {
            return Err(MaskJourneyOrderError::OutOfOrder {
                expected,
                observed: completed,
            });
        }
        self.next += 1;
        Ok(())
    }

    pub fn is_complete(&self) -> bool {
        self.next == MASK_JOURNEY_ACTIONS.len()
    }
}
