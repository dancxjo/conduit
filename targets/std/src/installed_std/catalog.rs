//! Exact installed implementation factory catalog.

mod support;
pub(crate) use support::supports;

use super::address_detect_back::FACTORY as ADDRESS_DETECT_FACTORY;
use super::alife_backs::{LENIA_STEP_FACTORY, ORBIUM_SEED_FACTORY, SCALAR_FIELD_FACE_FACTORY};
use super::audio_pitch_analysis_back::FACTORY as AUDIO_PITCH_ANALYSIS_FACTORY;
use super::audio_play_back::AUDIO_PLAY_FACTORY;
use super::audio_spectral_analysis_back::FACTORY as AUDIO_SPECTRAL_ANALYSIS_FACTORY;
use super::audio_time_analysis_back::FACTORY as AUDIO_TIME_ANALYSIS_FACTORY;
use super::audio_tone_back::FACTORY as AUDIO_TONE_FACTORY;
use super::back::BackFactory;
use super::body_chat_prompt_back::FACTORY as BODY_CHAT_PROMPT_FACTORY;
use super::body_conversation_context_back::FACTORY as BODY_CONVERSATION_CONTEXT_FACTORY;
use super::bool_face::BOOL_FACE_FACTORY;
use super::calendar_proposal_back::FACTORY as CALENDAR_PROPOSAL_FACTORY;
use super::calendar_provider_back::{
    CALENDAR_CANCEL_FACTORY, CALENDAR_CREATE_FACTORY, CALENDAR_FREE_BUSY_FACTORY,
    CALENDAR_INVITE_FACTORY, CALENDAR_READ_FACTORY, CALENDAR_UPDATE_FACTORY,
};
use super::combine_latest_back::FACTORY as COMBINE_LATEST_FACTORY;
use super::count_backs::{COUNT_FACE_FACTORY, STATE_COUNT_FACTORY};
use super::current_sample_back::FACTORY as CURRENT_SAMPLE_FACTORY;
use super::data_text_back::{
    FILE_LOAD_FACTORY as DATA_FILE_LOAD_TEXT_FACTORY,
    FILE_SAVE_FACTORY as DATA_FILE_SAVE_TEXT_FACTORY, LOAD_FACTORY as DATA_LOAD_TEXT_FACTORY,
    SAVE_FACTORY as DATA_SAVE_TEXT_FACTORY,
};
use super::distance_frequency_back::FACTORY as DISTANCE_FREQUENCY_FACTORY;
use super::external_websocket::EXTERNAL_WEBSOCKET_LISTENER_FACTORY;
use super::face_composition::{
    FACE_BADGE_FACTORY, FACE_FRAME_FACTORY, FACE_ICON_FACTORY, GRAPHICS_FACE_FACTORY,
    GRAPHICS_ICON_FACTORY, GRAPHICS_RECT_FACTORY, GRAPHICS_TEXT_FACTORY,
};
#[cfg(test)]
use super::face_composition::{TEST_FACE_SINK_FACTORY, TEST_GRAPHICS_SINK_FACTORY};
use super::final_normalized_pattern_back::FACTORY as FINAL_NORMALIZED_PATTERN_FACTORY;
use super::flow_collect_back::FACTORY as FLOW_COLLECT_FACTORY;
use super::flow_first_back::FLOW_FIRST_FACTORY;
use super::flow_gate_back::FLOW_GATE_SCALAR_FACTORY;
use super::flow_join_by_key_back::FACTORY as FLOW_JOIN_BY_KEY_FACTORY;
use super::flow_pressure_backs::{FLOW_BACKPRESSURE_FACTORY, FLOW_COALESCE_LATEST_FACTORY};
use super::flow_state_backs::{FLOW_TEE_SCALAR_FACTORY, STATE_LATEST_SCALAR_FACTORY};
use super::flow_zip_back::FACTORY as FLOW_ZIP_FACTORY;
use super::generated_speech_commit_back::FACTORY as GENERATED_SPEECH_COMMIT_FACTORY;
use super::generated_validation_backs::{
    ENVELOPE_FACTORY as GENERATED_VALIDATION_ENVELOPE_FACTORY,
    RETAIN_FACTORY as RETAIN_GENERATED_VALIDATION_FACTORY,
    VALIDATOR_FACTORY as GENERATED_SEMANTIC_VALIDATOR_FACTORY,
};
use super::house_prompt_back::FACTORY as HOUSE_PROMPT_FACTORY;
use super::http::{HTTP_CLIENT_FACTORY, HTTP_SERVER_FACTORY};
use super::image_text_back::FACTORY as IMAGE_TEXT_FACTORY;
use super::image_text_record_back::FACTORY as IMAGE_TEXT_RECORD_FACTORY;
use super::input_semantic_backs::{CHORDS_FACTORY, KEYMAP_FACTORY, KEY_EVENT_TEE_FACTORY};
use super::instrument_map_back::FACTORY as INSTRUMENT_MAP_FACTORY;
use super::json_backs::{JSON_COLLECTION_STEP_FACTORY, JSON_DECODE_FACTORY, JSON_ENCODE_FACTORY};
use super::json_snapshot_back::{
    PUBLISH_FACTORY as JSON_SNAPSHOT_PUBLISH_FACTORY, READ_FACTORY as JSON_SNAPSHOT_READ_FACTORY,
};
use super::json_summary_back::JSON_BOOLEAN_SUMMARY_FACTORY;
use super::keyboard_input_back::FACTORY as KEYBOARD_INPUT_FACTORY;
#[cfg(test)]
use super::layout_backs::TEST_LAYOUT_SINK_FACTORY;
use super::layout_backs::{
    LAYOUT_ALIGN_FACTORY, LAYOUT_COLUMN_FACTORY, LAYOUT_INSET_FACTORY, LAYOUT_ROW_FACTORY,
    LAYOUT_STACK_FACTORY, LAYOUT_VIEWPORT_FACTORY,
};
use super::local_model_back::LOCAL_MODEL_FACTORY;
use super::local_vision_back::{
    FACTORY as LOCAL_VISION_FACTORY, OCR_FACTORY as LOCAL_VISION_OCR_FACTORY,
    TRACK_FACTORY as LOCAL_VISION_TRACK_FACTORY,
};
use super::logic_backs::{
    LOGIC_COMPARE_SCALAR_FACTORY, LOGIC_NOT_FACTORY, LOGIC_SELECT_SCALAR_FACTORY,
};
use super::math_backs::{MATH_CLAMP_FACTORY, MATH_DEADBAND_FACTORY, MATH_SCALE_FACTORY};
use super::microphone_clip_back::{
    CONVERTED_FACTORY as CONVERTED_MICROPHONE_CLIP_FACTORY, FACTORY as MICROPHONE_CLIP_FACTORY,
};
use super::midi_input_back::MIDI_INPUT_FACTORY;
use super::midi_output_back::MIDI_OUTPUT_FACTORY;
use super::model_text_back::FACTORY as MODEL_TEXT_FACTORY;
use super::morse_backs::{
    INDICATOR_FACE_FACTORY, MORSE_FLATTEN_FACTORY, MORSE_INTERSPERSE_FACTORY, MORSE_LOOKUP_FACTORY,
    MORSE_SYMBOLS_TO_PATTERN_FACTORY, TEXT_CHARACTERS_FACTORY, TEXT_MORSE_FACTORY,
};
use super::navigation_backs::{
    CONTROL_FACTORY as NAVIGATION_CONTROL_FACTORY, ROUTE_FACTORY as NAVIGATION_ROUTE_FACTORY,
    TIME_FACTORY as NAVIGATION_TIME_FACTORY,
};
use super::pacing_backs::{TIME_DELAY_FACTORY, TIME_THROTTLE_FACTORY};
use super::pattern_comparison_back::FACTORY as PATTERN_COMPARISON_FACTORY;
use super::pcm_profile_conversion_back::FACTORY as PCM_PROFILE_CONVERSION_FACTORY;
use super::pure_expression_back::{
    FACTORY as PURE_EXPRESSION_FACTORY, FILTER_FACTORY as PURE_FILTER_FACTORY,
    LOCAL_FACTORY as PURE_LOCAL_EXPRESSION_FACTORY,
    SCALAR_TAPE_FACTORY as PURE_SCALAR_TAPE_FACTORY,
};
use super::recognition_text_back::FACTORY as RECOGNITION_TEXT_FACTORY;
use super::recognized_turn_commit_back::FACTORY as RECOGNIZED_TURN_COMMIT_FACTORY;
use super::record_delivery_back::FACTORY as RECORD_DELIVERY_STATUS_FACTORY;
use super::record_queue_back::FACTORY as RECORD_QUEUE_FACTORY;
use super::record_temporal_back::{
    EXACTLY_ONE as RECORD_EXACTLY_ONE_FACTORY, SINGLETON as RECORD_SINGLETON_STREAM_FACTORY,
};
use super::record_transcript_back::FACTORY as RECORD_TRANSCRIPT_FACTORY;
#[cfg(any(test, feature = "local-model-proof"))]
use super::recorded_speech_back::FACTORY as RECORDED_SPEECH_FACTORY;
use super::recurrence_back::FACTORY as RECURRENCE_FACTORY;
use super::render_demand_back::AUDIO_RENDER_DEMAND_FACTORY;
use super::rhythm_compare_back::FACTORY as RHYTHM_COMPARE_FACTORY;
use super::robotics_backs::{
    ROBOTICS_DRIVE_DIFFERENTIAL_FACTORY, ROBOTICS_OBSERVE_BATTERY_FACTORY,
    ROBOTICS_OBSERVE_BUMP_FACTORY, ROBOTICS_OBSERVE_IMU_FACTORY, ROBOTICS_OBSERVE_ODOMETRY_FACTORY,
    ROBOTICS_OBSERVE_RANGE_FACTORY, ROBOTICS_VELOCITY_INTENT_FACTORY,
};
use super::sequence_normalization_back::FACTORY as SEQUENCE_NORMALIZATION_FACTORY;
use super::speech_recognition_adapter_back::{
    RESULT_STREAM_FACTORY as SPEECH_RESULT_STREAM_FACTORY, WINDOW_FACTORY as SPEECH_WINDOW_FACTORY,
};
use super::speech_synthesis_back::DETERMINISTIC_FACTORY as DETERMINISTIC_SPEECH_FACTORY;
use super::speech_synthesis_back::DETERMINISTIC_STREAMING_FACTORY;
use super::spoken_mask_backs::{
    ARTIFACT_SHOW_FACTORY as SPOKEN_ARTIFACT_SHOW_FACTORY,
    FACE_REQUEST_FACTORY as SPOKEN_FACE_REQUEST_FACTORY,
    GENERATED_SPEECH_FACTORY as SPOKEN_GENERATED_SPEECH_FACTORY,
    NO_INTERACTION_FACTORY as SPOKEN_NO_INTERACTION_FACTORY, SPOKEN_ARTIFACT_FACTORY,
};
use super::state_select_back::STATE_SELECT_SCALAR_FACTORY;
use super::structured_selector_back::FACTORY as STRUCTURED_SELECTOR_FACTORY;
use super::structured_values_back::{
    FACE_FACTORY as STRUCTURED_FACE_FACTORY, LITERAL_FACTORY as STRUCTURED_LITERAL_FACTORY,
};
use super::synth_back::MUSIC_SYNTH_FACTORY;
use super::template_storage_back::FACTORY as TEMPLATE_STORAGE_FACTORY;
use super::test_audio_source::FACTORY as TEST_PCM_SOURCE_FACTORY;
#[cfg(test)]
use super::test_audio_tone::{
    CANCEL_FACTORY as TEST_CANCELLATION_SOURCE_FACTORY,
    CLOSE_FACTORY as TEST_NORMAL_CLOSE_SINK_FACTORY,
    DISTANCE_SOURCE_FACTORY as TEST_DISTANCE_SOURCE_FACTORY,
    RECOVERY_FACTORY as TEST_TONE_TERMINAL_RECOVERY_FACTORY,
    SINK_FACTORY as TEST_TONE_PCM_SINK_FACTORY, SOURCE_FACTORY as TEST_FREQUENCY_SOURCE_FACTORY,
};
#[cfg(test)]
use super::test_data_terminal_recovery::{
    LOAD_FACTORY as TEST_DATA_LOAD_TERMINAL_RECOVERY_FACTORY,
    SAVE_FACTORY as TEST_DATA_SAVE_TERMINAL_RECOVERY_FACTORY,
    TEXT_SINK_FACTORY as TEST_DATA_TEXT_SINK_FACTORY,
    TEXT_SOURCE_FACTORY as TEST_DATA_TEXT_SOURCE_FACTORY,
};
#[cfg(test)]
use super::test_gate::{TEST_GATE_SCRIPT_FACTORY, TEST_SLOW_SCALAR_SINK_FACTORY};
#[cfg(test)]
use super::test_input_semantics::{TEST_CHORD_SINK_FACTORY, TEST_KEY_EVENT_SOURCE_FACTORY};
#[cfg(test)]
use super::test_json_codec::{TEST_JSON_SINK_FACTORY, TEST_JSON_SOURCE_FACTORY};
#[cfg(any(test, feature = "local-model-proof"))]
use super::test_local_model_io::{TEST_LOCAL_MODEL_SINK_FACTORY, TEST_LOCAL_MODEL_SOURCE_FACTORY};
#[cfg(test)]
use super::test_logic::{TEST_LOGIC_SCRIPT_FACTORY, TEST_LOGIC_SINK_FACTORY};
#[cfg(test)]
use super::test_midi_source::FACTORY as TEST_MIDI_SOURCE_FACTORY;
#[cfg(test)]
use super::test_recurrence_sink::FACTORY as TEST_RECURRENCE_SINK_FACTORY;
#[cfg(test)]
use super::test_scalar_flow::{
    TEST_SCALAR_LITERAL_FACTORY, TEST_SCALAR_SINK_FACTORY, TEST_SCALAR_SOURCE_FACTORY,
};
#[cfg(any(test, feature = "local-model-proof"))]
use super::test_speech_sink::FACTORY as TEST_SPEECH_SINK_FACTORY;
#[cfg(test)]
use super::test_structured_selector::{
    SINK_FACTORY as TEST_STRUCTURED_SINK_FACTORY, SOURCE_FACTORY as TEST_STRUCTURED_SOURCE_FACTORY,
};
#[cfg(test)]
use super::test_text_source::TEST_TEXT_SOURCE_FACTORY;
#[cfg(test)]
use super::test_timing_sink::{
    TEST_TIMING_SINK_FACTORY, TEST_TIMING_SOURCE_FACTORY, TEST_TIMING_UNIT_SOURCE_FACTORY,
};
use super::text_backs::{
    TEXT_FACE_FACTORY, TEXT_JOIN_FACTORY, TEXT_LITERAL_FACTORY, TEXT_UPPER_FACTORY,
};
use super::text_state_back::{
    DOCUMENT_FACTORY as TEXT_DOCUMENT_FACTORY, EDIT_FACTORY as TEXT_EDIT_FACTORY,
    SUBMIT_FACTORY as TEXT_SUBMIT_FACTORY,
};
use super::thermostat_combine_back::FACTORY as THERMOSTAT_COMBINE_FACTORY;
#[cfg(test)]
use super::tick_backs::TEST_OBSERVER_FACTORY;
use super::tick_backs::{EVERY_FACTORY, TICK_FACTORY};
use super::tick_face::TICK_FACE_FACTORY;
use super::time_sample_back::FACTORY as TIME_SAMPLE_FACTORY;
use super::time_window_back::FACTORY as TIME_WINDOW_FACTORY;
use super::timed_button_attempt_back::FACTORY as TIMED_BUTTON_ATTEMPT_FACTORY;
use super::timed_pattern_back::FACTORY as TIMED_PATTERN_FACTORY;
use super::timing_backs::{TIME_DEADLINE_FACTORY, TIME_DEBOUNCE_FACTORY, TIME_TIMEOUT_FACTORY};
use super::todo_checkpoint_back::FACTORY as TODO_CHECKPOINT_FACTORY;
use super::todo_checkpoint_read_back::FACTORY as TODO_CHECKPOINT_READ_FACTORY;
use super::todo_combine_back::FACTORY as TODO_COMBINE_FACTORY;
use super::toggle_back::STATE_TOGGLE_FACTORY;
use super::typed_record_back::{
    DEFRAME as TYPED_RECORD_DEFRAME_FACTORY, FRAME as TYPED_RECORD_FRAME_FACTORY,
    RECORD_TO_TEXT as TYPED_RECORD_TO_TEXT_FACTORY, TEXT_TO_RECORD as TEXT_TO_TYPED_RECORD_FACTORY,
};
use super::vector_search_back::{EXACT_FACTORY as EXACT_VECTOR_SEARCH_FACTORY, HNSW_FACTORY};
use super::vision_describe_back::FACTORY as VISION_DESCRIBE_FACTORY;
use super::vision_experience_back::FACTORY as VISION_EXPERIENCE_FACTORY;
use super::wav_artifact_back::FACTORY as WAV_ARTIFACT_FACTORY;
use super::whisper_speech_back::{
    CLIP_FACTORY as WHISPER_CLIP_SPEECH_FACTORY, FACTORY as WHISPER_SPEECH_FACTORY,
};
use conduit_core::ImplementationId;

mod inventory;
use inventory::FACTORIES;

pub(super) fn factory(implementation_id: &ImplementationId) -> Option<&'static BackFactory> {
    FACTORIES
        .iter()
        .copied()
        .find(|factory| factory.implementation_id == implementation_id.as_str())
}
