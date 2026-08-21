//! Bounded receipts for renderer-neutral navigation enactment.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

use crate::{
    NavigationOperation, NavigationRefusal, NavigationState, Presentation, PresentationBasis,
    PresentationCursor, PresentationNavigation,
};

pub const NAVIGATION_JOURNEY_SCHEMA: &str =
    "conduit.presentation/renderer-navigation-journey-receipt@1";
pub const MAX_NAVIGATION_JOURNEY_STEPS: usize = 16;

/// The exact portable result of one attempted navigation operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NavigationJourneyDisposition {
    Advanced,
    Refused(NavigationRefusal),
}

/// One operation and its immutable semantic basis and cursor boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationJourneyStep {
    pub operation: NavigationOperation,
    pub disposition: NavigationJourneyDisposition,
    pub before_cursor: PresentationCursor,
    pub after_cursor: PresentationCursor,
    pub semantic_basis: PresentationBasis,
}

/// A finite renderer-neutral ledger. It contains no action invocation hook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigationJourneyReceipt {
    pub schema: String,
    pub maximum_steps: usize,
    pub presentation: crate::PresentationContentId,
    pub presentation_revision: u64,
    pub semantic_basis: PresentationBasis,
    pub history_limit: usize,
    pub start_cursor: PresentationCursor,
    pub terminal_cursor: PresentationCursor,
    pub steps: Vec<NavigationJourneyStep>,
}

impl NavigationJourneyReceipt {
    /// Re-enact the finite operation ledger and require byte-for-byte portable
    /// agreement before a consumer trusts a decoded receipt.
    pub fn validate(
        &self,
        presentation: &Presentation,
        navigation: &PresentationNavigation,
    ) -> Result<(), NavigationRefusal> {
        if self.schema != NAVIGATION_JOURNEY_SCHEMA
            || self.maximum_steps != MAX_NAVIGATION_JOURNEY_STEPS
            || self.steps.len() > MAX_NAVIGATION_JOURNEY_STEPS
            || self.presentation != presentation.identity
            || self.presentation_revision != presentation.revision
            || self.semantic_basis != presentation.basis
        {
            return Err(NavigationRefusal::InvalidTruth);
        }
        let operations = self
            .steps
            .iter()
            .map(|step| step.operation.clone())
            .collect::<Vec<_>>();
        let expected = enact_navigation_journey(
            presentation,
            navigation,
            self.start_cursor.clone(),
            self.history_limit,
            &operations,
        )?;
        if &expected == self {
            Ok(())
        } else {
            Err(NavigationRefusal::InvalidTruth)
        }
    }
}

/// Enact a finite sequence through the one shared pure transition boundary.
/// Refusals are retained and do not stop later independent attempts.
pub fn enact_navigation_journey(
    presentation: &Presentation,
    navigation: &PresentationNavigation,
    start_cursor: PresentationCursor,
    history_limit: usize,
    operations: &[NavigationOperation],
) -> Result<NavigationJourneyReceipt, NavigationRefusal> {
    if operations.len() > MAX_NAVIGATION_JOURNEY_STEPS {
        return Err(NavigationRefusal::InvalidTruth);
    }
    navigation.validate(presentation)?;
    let mut state = NavigationState::new(navigation, start_cursor.clone(), history_limit)?;
    let mut steps = Vec::with_capacity(operations.len());
    for operation in operations {
        let before_cursor = state.cursor().clone();
        let result = state
            .navigate(
                presentation,
                navigation,
                presentation.revision,
                operation.clone(),
            )
            .map(|_| NavigationJourneyDisposition::Advanced)
            .unwrap_or_else(NavigationJourneyDisposition::Refused);
        let after_cursor = state.cursor().clone();
        if matches!(result, NavigationJourneyDisposition::Refused(_))
            && after_cursor != before_cursor
        {
            return Err(NavigationRefusal::InvalidTruth);
        }
        steps.push(NavigationJourneyStep {
            operation: operation.clone(),
            disposition: result,
            before_cursor,
            after_cursor,
            semantic_basis: presentation.basis.clone(),
        });
    }
    Ok(NavigationJourneyReceipt {
        schema: NAVIGATION_JOURNEY_SCHEMA.to_string(),
        maximum_steps: MAX_NAVIGATION_JOURNEY_STEPS,
        presentation: presentation.identity.clone(),
        presentation_revision: presentation.revision,
        semantic_basis: presentation.basis.clone(),
        history_limit,
        start_cursor,
        terminal_cursor: state.cursor().clone(),
        steps,
    })
}
