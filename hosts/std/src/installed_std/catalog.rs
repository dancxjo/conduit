//! Exact installed implementation factory catalog.

use super::audio_play_operation::AUDIO_PLAY_FACTORY;
use super::bool_presentation::BOOL_PRESENTATION_FACTORY;
use super::count_operations::{COUNT_PRESENTATION_FACTORY, STATE_COUNT_FACTORY};
use super::external_websocket::EXTERNAL_WEBSOCKET_LISTENER_FACTORY;
use super::flow_gate_operation::FLOW_GATE_SCALAR_FACTORY;
use super::flow_state_operations::{FLOW_TEE_SCALAR_FACTORY, STATE_LATEST_SCALAR_FACTORY};
use super::generate_text::{
    GENERATE_TEXT_LARGE_FACTORY, GENERATE_TEXT_REMOTE_FACTORY, GENERATE_TEXT_SMALL_FACTORY,
};
use super::http::{HTTP_CLIENT_FACTORY, HTTP_SERVER_FACTORY};
use super::input_semantic_operations::{CHORDS_FACTORY, KEYMAP_FACTORY, KEY_EVENT_TEE_FACTORY};
use super::json_operations::{JSON_DECODE_FACTORY, JSON_ENCODE_FACTORY};
use super::keyboard_input_operation::FACTORY as KEYBOARD_INPUT_FACTORY;
#[cfg(test)]
use super::layout_operations::TEST_LAYOUT_SINK_FACTORY;
use super::layout_operations::{
    LAYOUT_ALIGN_FACTORY, LAYOUT_COLUMN_FACTORY, LAYOUT_INSET_FACTORY, LAYOUT_ROW_FACTORY,
    LAYOUT_STACK_FACTORY, LAYOUT_VIEWPORT_FACTORY,
};
use super::logic_operations::{
    LOGIC_COMPARE_SCALAR_FACTORY, LOGIC_NOT_FACTORY, LOGIC_SELECT_SCALAR_FACTORY,
};
use super::math_operations::{MATH_CLAMP_FACTORY, MATH_DEADBAND_FACTORY, MATH_SCALE_FACTORY};
use super::midi_input_operation::MIDI_INPUT_FACTORY;
use super::midi_output_operation::MIDI_OUTPUT_FACTORY;
use super::operation::InstalledFactory;
use super::pacing_operations::{TIME_DELAY_FACTORY, TIME_THROTTLE_FACTORY};
use super::presentation_composition::{
    GRAPHICS_ICON_FACTORY, GRAPHICS_PRESENTATION_FACTORY, GRAPHICS_RECT_FACTORY,
    GRAPHICS_TEXT_FACTORY, PRESENTATION_BADGE_FACTORY, PRESENTATION_FRAME_FACTORY,
    PRESENTATION_ICON_FACTORY,
};
#[cfg(test)]
use super::presentation_composition::{TEST_GRAPHICS_SINK_FACTORY, TEST_PRESENTATION_SINK_FACTORY};
use super::render_demand_operation::AUDIO_RENDER_DEMAND_FACTORY;
use super::robotics_operations::{
    ROBOTICS_DRIVE_DIFFERENTIAL_FACTORY, ROBOTICS_OBSERVE_BATTERY_FACTORY,
    ROBOTICS_OBSERVE_BUMP_FACTORY, ROBOTICS_OBSERVE_IMU_FACTORY, ROBOTICS_OBSERVE_ODOMETRY_FACTORY,
    ROBOTICS_OBSERVE_RANGE_FACTORY, ROBOTICS_VELOCITY_INTENT_FACTORY,
};
use super::synth_operation::MUSIC_SYNTH_FACTORY;
use super::test_audio_source::FACTORY as TEST_PCM_SOURCE_FACTORY;
#[cfg(test)]
use super::test_gate::{TEST_GATE_SCRIPT_FACTORY, TEST_SLOW_SCALAR_SINK_FACTORY};
#[cfg(test)]
use super::test_input_semantics::{TEST_CHORD_SINK_FACTORY, TEST_KEY_EVENT_SOURCE_FACTORY};
#[cfg(test)]
use super::test_json_codec::{TEST_JSON_SINK_FACTORY, TEST_JSON_SOURCE_FACTORY};
#[cfg(test)]
use super::test_logic::{TEST_LOGIC_SCRIPT_FACTORY, TEST_LOGIC_SINK_FACTORY};
#[cfg(test)]
use super::test_midi_source::FACTORY as TEST_MIDI_SOURCE_FACTORY;
#[cfg(test)]
use super::test_scalar_flow::{
    TEST_SCALAR_LITERAL_FACTORY, TEST_SCALAR_SINK_FACTORY, TEST_SCALAR_SOURCE_FACTORY,
};
#[cfg(test)]
use super::test_text_source::TEST_TEXT_SOURCE_FACTORY;
#[cfg(test)]
use super::test_timing_sink::{TEST_TIMING_SINK_FACTORY, TEST_TIMING_SOURCE_FACTORY};
use super::text_operations::{
    TEXT_JOIN_FACTORY, TEXT_LITERAL_FACTORY, TEXT_PRESENTATION_FACTORY, TEXT_UPPER_FACTORY,
};
#[cfg(test)]
use super::tick_operations::TEST_OBSERVER_FACTORY;
use super::tick_operations::{EVERY_FACTORY, TICK_FACTORY};
use super::tick_presentation::TICK_PRESENTATION_FACTORY;
use super::timing_operations::{TIME_DEBOUNCE_FACTORY, TIME_TIMEOUT_FACTORY};
use super::toggle_operation::STATE_TOGGLE_FACTORY;
use conduit_core::{ImplementationId, PlanFragment};

const FACTORIES: &[&InstalledFactory] = &[
    &KEYBOARD_INPUT_FACTORY,
    &TICK_FACTORY,
    &EVERY_FACTORY,
    &TIME_DEBOUNCE_FACTORY,
    &TIME_TIMEOUT_FACTORY,
    &TIME_DELAY_FACTORY,
    &TIME_THROTTLE_FACTORY,
    &TICK_PRESENTATION_FACTORY,
    &BOOL_PRESENTATION_FACTORY,
    &TEXT_LITERAL_FACTORY,
    &TEXT_UPPER_FACTORY,
    &TEXT_JOIN_FACTORY,
    &TEXT_PRESENTATION_FACTORY,
    &STATE_COUNT_FACTORY,
    &STATE_TOGGLE_FACTORY,
    &COUNT_PRESENTATION_FACTORY,
    &STATE_LATEST_SCALAR_FACTORY,
    &FLOW_TEE_SCALAR_FACTORY,
    &FLOW_GATE_SCALAR_FACTORY,
    &KEY_EVENT_TEE_FACTORY,
    &KEYMAP_FACTORY,
    &CHORDS_FACTORY,
    &LOGIC_COMPARE_SCALAR_FACTORY,
    &LOGIC_NOT_FACTORY,
    &LOGIC_SELECT_SCALAR_FACTORY,
    &MATH_CLAMP_FACTORY,
    &MATH_SCALE_FACTORY,
    &MATH_DEADBAND_FACTORY,
    &LAYOUT_VIEWPORT_FACTORY,
    &LAYOUT_INSET_FACTORY,
    &LAYOUT_ROW_FACTORY,
    &LAYOUT_COLUMN_FACTORY,
    &LAYOUT_STACK_FACTORY,
    &LAYOUT_ALIGN_FACTORY,
    &PRESENTATION_ICON_FACTORY,
    &PRESENTATION_FRAME_FACTORY,
    &PRESENTATION_BADGE_FACTORY,
    &GRAPHICS_RECT_FACTORY,
    &GRAPHICS_TEXT_FACTORY,
    &GRAPHICS_ICON_FACTORY,
    &GRAPHICS_PRESENTATION_FACTORY,
    #[cfg(test)]
    &TEST_PRESENTATION_SINK_FACTORY,
    #[cfg(test)]
    &TEST_GRAPHICS_SINK_FACTORY,
    #[cfg(test)]
    &TEST_LAYOUT_SINK_FACTORY,
    &ROBOTICS_OBSERVE_BUMP_FACTORY,
    &ROBOTICS_OBSERVE_IMU_FACTORY,
    &ROBOTICS_OBSERVE_RANGE_FACTORY,
    &ROBOTICS_OBSERVE_ODOMETRY_FACTORY,
    &ROBOTICS_OBSERVE_BATTERY_FACTORY,
    &ROBOTICS_VELOCITY_INTENT_FACTORY,
    &ROBOTICS_DRIVE_DIFFERENTIAL_FACTORY,
    &MUSIC_SYNTH_FACTORY,
    &AUDIO_RENDER_DEMAND_FACTORY,
    &AUDIO_PLAY_FACTORY,
    &MIDI_OUTPUT_FACTORY,
    &MIDI_INPUT_FACTORY,
    &EXTERNAL_WEBSOCKET_LISTENER_FACTORY,
    &GENERATE_TEXT_SMALL_FACTORY,
    &GENERATE_TEXT_LARGE_FACTORY,
    &GENERATE_TEXT_REMOTE_FACTORY,
    &HTTP_CLIENT_FACTORY,
    &HTTP_SERVER_FACTORY,
    &JSON_ENCODE_FACTORY,
    &JSON_DECODE_FACTORY,
    #[cfg(test)]
    &TEST_TEXT_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_MIDI_SOURCE_FACTORY,
    &TEST_PCM_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_LITERAL_FACTORY,
    #[cfg(test)]
    &TEST_SCALAR_SINK_FACTORY,
    #[cfg(test)]
    &TEST_GATE_SCRIPT_FACTORY,
    #[cfg(test)]
    &TEST_KEY_EVENT_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_CHORD_SINK_FACTORY,
    #[cfg(test)]
    &TEST_LOGIC_SCRIPT_FACTORY,
    #[cfg(test)]
    &TEST_LOGIC_SINK_FACTORY,
    #[cfg(test)]
    &TEST_SLOW_SCALAR_SINK_FACTORY,
    #[cfg(test)]
    &TEST_TIMING_SINK_FACTORY,
    #[cfg(test)]
    &TEST_TIMING_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_JSON_SOURCE_FACTORY,
    #[cfg(test)]
    &TEST_JSON_SINK_FACTORY,
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
