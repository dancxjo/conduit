//! Exact installed implementation factory catalog.

use super::count_operations::{COUNT_PRESENTATION_FACTORY, STATE_COUNT_FACTORY};
use super::external_websocket::EXTERNAL_WEBSOCKET_LISTENER_FACTORY;
use super::generate_text::{
    GENERATE_TEXT_LARGE_FACTORY, GENERATE_TEXT_REMOTE_FACTORY, GENERATE_TEXT_SMALL_FACTORY,
};
#[cfg(test)]
use super::operation::TEST_OBSERVER_FACTORY;
use super::operation::{InstalledFactory, EVERY_FACTORY, TICK_FACTORY};
#[cfg(test)]
use super::test_text_source::TEST_TEXT_SOURCE_FACTORY;
use super::text_operations::{
    TEXT_JOIN_FACTORY, TEXT_LITERAL_FACTORY, TEXT_PRESENTATION_FACTORY, TEXT_UPPER_FACTORY,
};
use super::tick_presentation::TICK_PRESENTATION_FACTORY;
use conduit_core::{ImplementationId, PlanFragment};

const FACTORIES: &[&InstalledFactory] = &[
    &TICK_FACTORY,
    &EVERY_FACTORY,
    &TICK_PRESENTATION_FACTORY,
    &TEXT_LITERAL_FACTORY,
    &TEXT_UPPER_FACTORY,
    &TEXT_JOIN_FACTORY,
    &TEXT_PRESENTATION_FACTORY,
    &STATE_COUNT_FACTORY,
    &COUNT_PRESENTATION_FACTORY,
    &EXTERNAL_WEBSOCKET_LISTENER_FACTORY,
    &GENERATE_TEXT_SMALL_FACTORY,
    &GENERATE_TEXT_LARGE_FACTORY,
    &GENERATE_TEXT_REMOTE_FACTORY,
    #[cfg(test)]
    &TEST_TEXT_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_OBSERVER_FACTORY,
];

pub(super) fn factory(implementation_id: &ImplementationId) -> Option<&'static InstalledFactory> {
    FACTORIES
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}

pub(crate) fn supports(fragment: &PlanFragment) -> bool {
    !fragment.placements.is_empty()
        && fragment
            .placements
            .iter()
            .all(|placement| factory(&placement.implementation_id).is_some())
}
