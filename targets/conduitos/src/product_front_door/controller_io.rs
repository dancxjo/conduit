//! Finite values crossing the product controller's Root adapter boundary.
//! These values carry no physical device handles or proven-local rescue grant.

pub(super) use crate::keyboard_input::ProductInputControl as ControllerControl;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ControllerInput {
    Service,
    Key(conduit_human::KeyEvent),
    Lost(&'static str),
    Pointer(conduit_semantic_catalog::NormalizedPointerSample),
    PointerLost(&'static str),
}

/// Request the existing checked canonical stage preparation and execution.
/// Coordinates are validated by tour_play before any machine execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RunTourStage {
    pub chapter: u8,
    pub stage: u8,
}
