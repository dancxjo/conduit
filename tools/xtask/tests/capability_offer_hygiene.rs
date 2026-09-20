use std::{fs, path::Path};

/// Production constructors already migrated under #3718. Once a path enters
/// this list it cannot regain a raw `CapabilityOffer` literal; malformed and
/// adversarial fixtures remain outside this production-only ratchet.
const MIGRATED_PRODUCTION_PATHS: &[(&str, usize, &str)] = &[
    (
        "semantics/catalog/src/final_normalized_pattern.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/pattern_comparison.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/functional_front.rs",
        0,
        "fully migrated",
    ),
    (
        "semantics/catalog/src/sequence_normalization.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/timed_pattern.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/template_storage.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/ordered_record_queue_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/record_delivery_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/record_temporal_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/record_transcript_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/net/src/typed_record_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/time/src/catalog.rs",
        0,
        "semantic contract owner",
    ),
    ("semantics/text/src/morse.rs", 0, "semantic contract owner"),
    ("semantics/text/src/lib.rs", 0, "semantic contract owner"),
    (
        "semantics/data/src/measurement_plot_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/data/src/measurement_threshold_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/data/src/measurement_summary_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/structured_selector.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/structured_values.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/flow_pressure.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/education_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/vision_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/robotics_structured_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/structured_music_form.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/human_media_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/job_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/reminder_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/json.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/flow_state.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/button_indicator.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/state_count.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/state_toggle.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/input_semantics.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/timing.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/time_every.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/tick.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/audio_render_demand.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/navigation_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/timed_button_attempt.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/quantity_info.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/quantity_mapping.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/normalized_quantity.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/text_state.rs",
        0,
        "semantic contract owner",
    ),
    ("semantics/signal/src/control.rs", 0, "fully migrated"),
    ("semantics/signal/src/lib.rs", 0, "fully migrated"),
    ("semantics/tongues/src/contract.rs", 0, "fully migrated"),
    (
        "semantics/tongues/src/speech_recognition.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/tongues/src/recognition_adapters.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/tongues/src/house_conversation.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/chat/src/body_chat.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/human_media_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/browser_human_io.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/keyboard.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/generalized_input_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/recurrence_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/calendar_proposal_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/ai/src/model_text.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/vision_catalog.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/alife.rs",
        0,
        "semantic contract owner",
    ),
    (
        "semantics/catalog/src/robotics.rs",
        0,
        "semantic contract owner",
    ),
    (
        "targets/browser/runtime/src/installed_browser/audio_io.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/final_normalized_pattern.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/json.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/pattern_comparison.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/timing.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/template_storage.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/text_state.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/typed_record.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_delivery.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/pulse_observation.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/morse_composition.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/normalized_quantity.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/structured_selector.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_queue.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_temporal.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/record_transcript.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/structured_offers.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/presentation_nucleus/text_offer.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/human_media/offers.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/button_attempt.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/morse.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/measurement_presentation.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/measurement_summary.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/measurement_plot.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/copy_file.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/final_normalized_pattern.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/json.rs", 0, "fully migrated"),
    ("targets/std/offers/src/flow_state.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/keyboard/button.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/state_input.rs", 0, "fully migrated"),
    ("targets/std/offers/src/timing.rs", 0, "fully migrated"),
    ("targets/std/offers/src/navigation.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/timed_button_attempt.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/conversation_commit.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/speech_recognition.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/speech_recognition_adapters.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/house_conversation.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/body_chat.rs", 0, "fully migrated"),
    ("targets/std/offers/src/microphone.rs", 0, "fully migrated"),
    ("targets/std/offers/src/image_text.rs", 0, "fully migrated"),
    ("targets/std/offers/src/keyboard.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/generalized_input.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/calendar.rs", 0, "fully migrated"),
    ("targets/std/offers/src/model_text.rs", 0, "fully migrated"),
    ("targets/std/offers/src/vision.rs", 0, "fully migrated"),
    ("targets/std/offers/src/alife.rs", 0, "fully migrated"),
    ("targets/std/offers/src/robotics.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/quantity_info.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/quantity_mapping.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/pattern_comparison.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/music.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/record_delivery.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/record_queue.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/record_temporal.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/record_transcript.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/signal.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/sequence_normalization.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/speech_synthesis.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/speech_commit.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/timed_pattern.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/template_storage.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/text_state.rs", 0, "fully migrated"),
    ("targets/std/offers/src/text.rs", 0, "fully migrated"),
    (
        "targets/std/offers/src/typed_record.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/pulse_observation.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/morse_composition.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/structured_selector.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/structured_values.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/structured_values/flow_pressure.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/offers/src/domain_specimens.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/offers/src/workflows.rs", 0, "fully migrated"),
];

#[test]
fn migrated_production_offers_cannot_restate_capability_truth() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");

    let mut violations = Vec::new();
    for (relative, expected_raw_literals, reason) in MIGRATED_PRODUCTION_PATHS {
        let source = fs::read_to_string(repository.join(relative))
            .unwrap_or_else(|error| panic!("read {relative}: {error}"));
        let raw_literals = source
            .lines()
            .filter(|line| {
                line.contains("CapabilityOffer {")
                    && !line.contains("-> CapabilityOffer {")
                    && !line.contains("struct CapabilityOffer {")
            })
            .count();
        if raw_literals != *expected_raw_literals {
            violations.push(format!(
                "{relative}: expected {expected_raw_literals} reviewed raw literal(s) ({reason}), found {raw_literals}; update the ratchet when migrating debt"
            ));
        }
        if !source.contains("CapabilityOfferBuilder")
            && !source.contains("realization_offer")
            && !source.contains("SemanticCapabilityContract")
        {
            violations.push(format!(
                "{relative}: lost the canonical capability-offer construction path"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "migrated production capability offers regressed:\n{}",
        violations.join("\n")
    );
}
