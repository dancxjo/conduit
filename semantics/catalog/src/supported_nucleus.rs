//! Exact typed contracts and offers installed by the executable std nucleus.

use super::*;

/// Exact typed contracts currently supported by the executable `conduit.std` nucleus.
///
/// Conduit v1 exposes only these exact typed revisions; the former erased
/// `value/any` catalog is not compiled or discoverable.
pub fn supported_nucleus_contracts() -> Vec<StandardKindContract> {
    supported_nucleus_contracts_with_revisions()
        .into_iter()
        .map(|(contract, _)| contract)
        .collect()
}

/// Portable contract definitions paired with their exact semantic revision.
///
/// Revision truth belongs with the contract inventory. Profile construction
/// must not recover it from any Host's implementation offers.
pub(crate) fn supported_nucleus_contracts_with_revisions(
) -> Vec<(StandardKindContract, &'static str)> {
    vec![
        (tick_contract(), "conduit.std/time-tick@2"),
        (time_every_contract(), "conduit.std/time-every@1"),
        (
            audio_render_demand_contract(),
            "conduit.std/audio-render-demand@1",
        ),
        (music_synth_contract(), "conduit.std/music-synth@1"),
        (time_debounce_contract(), "conduit.std/time-debounce-bool@1"),
        (
            time_timeout_contract(),
            "conduit.std/time-timeout-tick-bool@1",
        ),
        (time_delay_contract(), "conduit.std/time-delay-bool@1"),
        (
            time_throttle_contract(),
            "conduit.std/time-throttle-bool-leading@1",
        ),
        (
            tick_presentation_contract(),
            "conduit.std/presentation-tick@1",
        ),
        (bool_presentation_contract(), "conduit.presentation/bool@1"),
        (text_literal_contract(), "conduit.std/text-literal@1"),
        (text_upper_contract(), "conduit.std/text-upper@1"),
        (text_join_contract(), "conduit.std/text-join@1"),
        (
            text_presentation_contract(),
            "conduit.std/presentation-text@1",
        ),
        (key_event_tee_contract(), "conduit.input/key-tee@1"),
        (keymap_contract(), "conduit.input/keymap@1"),
        (chords_contract(), "conduit.input/chords@1"),
        (state_count_contract(), "conduit.std/state-count@1"),
        (state_toggle_contract(), "conduit.std/state-toggle@1"),
        (
            count_presentation_contract(),
            "conduit.std/presentation-count@1",
        ),
        (
            state_latest_scalar_contract(),
            "conduit.std/state-latest-scalar@2",
        ),
        (flow_tee_scalar_contract(), "conduit.std/flow-tee-scalar@2"),
        (
            flow_gate_scalar_contract(),
            "conduit.std/flow-gate-scalar@1",
        ),
        (
            state_select_scalar_contract(),
            "conduit.std/state-select-scalar@1",
        ),
        (
            logic_compare_scalar_contract(),
            "conduit.std/logic-compare-scalar@1",
        ),
        (logic_not_contract(), "conduit.std/logic-not@1"),
        (
            logic_select_scalar_contract(),
            "conduit.std/logic-select-scalar@1",
        ),
        (math_clamp_contract(), "conduit.std/math-clamp-scalar@1"),
        (math_scale_contract(), "conduit.std/math-scale-scalar@1"),
        (
            math_deadband_contract(),
            "conduit.std/math-deadband-scalar@1",
        ),
        (quantity_map_contract(), QUANTITY_MAP_REVISION),
        (quantity_info_wrap_contract(), QUANTITY_INFO_WRAP_REVISION),
        (layout_viewport_contract(), "conduit.std/layout-frame@1"),
        (layout_inset_contract(), "conduit.std/layout-frame@1"),
        (layout_row_contract(), "conduit.std/layout-frame@1"),
        (layout_column_contract(), "conduit.std/layout-frame@1"),
        (layout_stack_contract(), "conduit.std/layout-frame@1"),
        (layout_align_contract(), "conduit.std/layout-frame@1"),
        (
            presentation_icon_contract(),
            "conduit.std/presentation-composition@1",
        ),
        (
            presentation_frame_contract(),
            "conduit.std/presentation-composition@1",
        ),
        (
            presentation_badge_contract(),
            "conduit.std/presentation-composition@1",
        ),
        (graphics_rect_contract(), "conduit.std/graphics-scene@1"),
        (graphics_text_contract(), "conduit.std/graphics-scene@1"),
        (graphics_icon_contract(), "conduit.std/graphics-scene@1"),
        (
            graphics_presentation_contract(),
            "conduit.std/presentation-graphics@1",
        ),
        (
            bitmap_presentation_contract(),
            "conduit.presentation/bitmap@1",
        ),
        (
            robotics_observe_bump_contract(),
            "conduit.std/robotics-observe-bump@1",
        ),
        (
            robotics_observe_imu_contract(),
            "conduit.std/robotics-observe-imu@1",
        ),
        (
            robotics_observe_range_contract(),
            "conduit.std/robotics-observe-range@1",
        ),
        (
            robotics_observe_odometry_contract(),
            "conduit.std/robotics-observe-odometry@1",
        ),
        (
            robotics_observe_battery_contract(),
            "conduit.std/robotics-observe-battery@1",
        ),
        (
            robotics_velocity_intent_contract(),
            "conduit.std/robotics-velocity-intent@1",
        ),
        (
            robotics_drive_differential_contract(),
            "conduit.std/robotics-drive-differential@2",
        ),
        (copy_file_contract(), "conduit.std/file-copy@1"),
        (json_encode_contract(), "conduit.std/json-encode@1"),
        (json_decode_contract(), "conduit.std/json-decode@1"),
    ]
}
