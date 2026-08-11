//! Exact installed implementation factory catalog.

use super::count_operations::{COUNT_PRESENTATION_FACTORY, STATE_COUNT_FACTORY};
use super::external_websocket::EXTERNAL_WEBSOCKET_LISTENER_FACTORY;
use super::flow_gate_operation::FLOW_GATE_SCALAR_FACTORY;
use super::flow_state_operations::{FLOW_TEE_SCALAR_FACTORY, STATE_LATEST_SCALAR_FACTORY};
use super::generate_text::{
    GENERATE_TEXT_LARGE_FACTORY, GENERATE_TEXT_REMOTE_FACTORY, GENERATE_TEXT_SMALL_FACTORY,
};
use super::logic_operations::{
    LOGIC_COMPARE_SCALAR_FACTORY, LOGIC_NOT_FACTORY, LOGIC_SELECT_SCALAR_FACTORY,
};
use super::math_operations::{MATH_CLAMP_FACTORY, MATH_DEADBAND_FACTORY, MATH_SCALE_FACTORY};
#[cfg(test)]
use super::operation::TEST_OBSERVER_FACTORY;
use super::operation::{InstalledFactory, EVERY_FACTORY, TICK_FACTORY};
#[cfg(test)]
use super::test_gate::{TEST_GATE_SCRIPT_FACTORY, TEST_SLOW_SCALAR_SINK_FACTORY};
#[cfg(test)]
use super::test_logic::{TEST_LOGIC_SCRIPT_FACTORY, TEST_LOGIC_SINK_FACTORY};
#[cfg(test)]
use super::test_scalar_flow::{
    TEST_SCALAR_LITERAL_FACTORY, TEST_SCALAR_SINK_FACTORY, TEST_SCALAR_SOURCE_FACTORY,
};
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
    &STATE_LATEST_SCALAR_FACTORY,
    &FLOW_TEE_SCALAR_FACTORY,
    &FLOW_GATE_SCALAR_FACTORY,
    &LOGIC_COMPARE_SCALAR_FACTORY,
    &LOGIC_NOT_FACTORY,
    &LOGIC_SELECT_SCALAR_FACTORY,
    &MATH_CLAMP_FACTORY,
    &MATH_SCALE_FACTORY,
    &MATH_DEADBAND_FACTORY,
    &EXTERNAL_WEBSOCKET_LISTENER_FACTORY,
    &GENERATE_TEXT_SMALL_FACTORY,
    &GENERATE_TEXT_LARGE_FACTORY,
    &GENERATE_TEXT_REMOTE_FACTORY,
    #[cfg(test)]
    &TEST_TEXT_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_LITERAL_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_SINK_FACTORY,
    #[cfg(test)]
    &TEST_GATE_SCRIPT_FACTORY,
    #[cfg(test)]
    &TEST_LOGIC_SCRIPT_FACTORY,
    #[cfg(test)]
    &TEST_LOGIC_SINK_FACTORY,
    #[cfg(test)]
    &TEST_SLOW_SCALAR_SINK_FACTORY,
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
