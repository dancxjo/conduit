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
        "semantics/data/src/measurement_window_catalog.rs",
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
        "semantics/robotics/src/structured.rs",
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
        "targets/browser/runtime/src/installed_browser/quantity_output.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/browser_pointer.rs",
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
    (
        "targets/browser/runtime/src/installed_browser/measurement_window.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/measurement_hysteresis.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/rhythm_state.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/phase_synchronization.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/replay_source.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/replay_control.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/historical.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/deterministic_garden_source.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/garden_step.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/presentation.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/pointer_selector.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/little_seismograph_source.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/stroke_capture.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/browser/runtime/src/installed_browser/linguistics.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/conduitos/src/functional_offers.rs",
        0,
        "fully migrated",
    ),
    ("targets/conduitos/src/http/offer.rs", 0, "fully migrated"),
    ("bodies/pete/src/imu_observation.rs", 0, "fully migrated"),
    (
        "bodies/pete/src/create_observation_offer.rs",
        0,
        "fully migrated",
    ),
    ("semantics/ai/src/local_model.rs", 0, "fully migrated"),
    ("semantics/ai/src/provider.rs", 0, "fully migrated"),
    (
        "semantics/chat/src/interactive_catalog.rs",
        0,
        "fully migrated",
    ),
    ("semantics/chat/src/shared_pool.rs", 0, "fully migrated"),
    ("semantics/net/src/lib.rs", 0, "fully migrated"),
    (
        "semantics/net/src/external_websocket.rs",
        0,
        "fully migrated",
    ),
    (
        "semantics/net/src/application_catalog.rs",
        0,
        "fully migrated",
    ),
    (
        "semantics/system-continuity/src/reboot.rs",
        0,
        "fully migrated",
    ),
    ("semantics/tongues/src/realization.rs", 0, "fully migrated"),
    (
        "semantics/catalog/src/text_lab_plan.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/src/hosted_linguistics.rs", 0, "fully migrated"),
    ("targets/std/src/hosted_data.rs", 0, "fully migrated"),
    ("targets/std/src/hosted_geometry.rs", 0, "fully migrated"),
    ("targets/std/src/reaction_diffusion.rs", 0, "fully migrated"),
    (
        "targets/std/src/hosted_messaging/offer.rs",
        0,
        "fully migrated",
    ),
    (
        "targets/std/src/hosted_calendar/plan_contract.rs",
        0,
        "fully migrated",
    ),
    ("targets/std/src/installed_std/http.rs", 0, "fully migrated"),
    (
        "targets/std/src/installed_std/recorded_speech_back.rs",
        0,
        "fully migrated",
    ),
    (
        "semantics/presentation/src/contract.rs",
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
        if !source.contains("BackOfferBuilder")
            && !source.contains("realization_offer")
            && !source.contains("Kind")
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

#[test]
fn portable_capability_offers_cannot_bypass_the_checked_construction_boundary() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let permitted = [
        ("architecture/core/src/capability_offer.rs", 2usize),
        // This is ConduitOS's separate allocation-free machine record, not
        // conduit_core::CapabilityOffer.
        ("targets/conduitos/src/offer.rs", 2),
        ("targets/conduitos/src/text_offer.rs", 6),
        ("targets/conduitos/src/tour_timer_offer.rs", 3),
    ];
    let mut files = Vec::new();
    collect_rust_files(repository, &mut files);
    let mut violations = Vec::new();
    for path in files {
        let relative = path.strip_prefix(repository).unwrap();
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", relative.display()));
        let lines = source.lines().collect::<Vec<_>>();
        let actual = lines
            .iter()
            .enumerate()
            .filter(|(index, line)| {
                let exact_name = line.match_indices("CapabilityOffer {").any(|(offset, _)| {
                    offset == 0
                        || !line.as_bytes()[offset - 1].is_ascii_alphanumeric()
                            && line.as_bytes()[offset - 1] != b'_'
                });
                if !exact_name
                    || line.contains("struct CapabilityOffer {")
                    || line.contains("impl CapabilityOffer {")
                {
                    return false;
                }
                let first_body_line = lines[index + 1..]
                    .iter()
                    .find(|candidate| !candidate.trim().is_empty())
                    .map(|candidate| candidate.trim())
                    .unwrap_or_default();
                [
                    "startup_parameters:",
                    "semantic_contract:",
                    "capability_id:",
                    "kind:",
                ]
                .iter()
                .any(|field| first_body_line.starts_with(field))
                    || line.contains("$($fields)*")
            })
            .count();
        let expected = permitted
            .iter()
            .find_map(|(name, count)| (relative == Path::new(name)).then_some(*count))
            .unwrap_or(0);
        if actual != expected {
            violations.push(format!(
                "{}: expected {expected} raw literal(s), found {actual}",
                relative.display()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "portable capability construction escaped its reviewed boundary:\n{}",
        violations.join("\n")
    );
}

#[test]
fn checked_and_planned_gears_cannot_bypass_their_construction_boundaries() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("xtask is beneath repository tools");
    let mut files = Vec::new();
    collect_rust_files(repository, &mut files);
    for (type_name, owner) in [
        ("PlannedGear", "architecture/core/src/planned_gear.rs"),
        ("CheckedGear", "architecture/form/src/functional_front.rs"),
    ] {
        let mut raw = Vec::new();
        for path in &files {
            let relative = path.strip_prefix(repository).unwrap();
            let source = fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("read {}: {error}", relative.display()));
            let lines = source.lines().collect::<Vec<_>>();
            for (line_number, line) in lines.iter().enumerate() {
                let needle = format!("{type_name} {{");
                let exact_name = line.match_indices(&needle).any(|(offset, _)| {
                    offset == 0
                        || !line.as_bytes()[offset - 1].is_ascii_alphanumeric()
                            && line.as_bytes()[offset - 1] != b'_'
                });
                let first_body_line = lines[line_number + 1..]
                    .iter()
                    .find(|candidate| !candidate.trim().is_empty())
                    .map(|candidate| candidate.trim())
                    .unwrap_or_default();
                let begins_record = ["semantic_contract:", "placement_id:", "gear_id:"]
                    .iter()
                    .any(|field| first_body_line.starts_with(field));
                if exact_name
                    && (begins_record || line.contains("$crate::"))
                    && !line.contains(&format!("struct {type_name} {{"))
                    && !line.contains(&format!("impl {type_name} {{"))
                    && !line.contains(&format!("-> {type_name} {{"))
                    && !(relative == Path::new(owner) && line.contains("$crate::"))
                {
                    raw.push(format!("{}:{}", relative.display(), line_number + 1));
                }
            }
        }
        assert!(
            raw.is_empty(),
            "{type_name} construction escaped its reviewed boundary:\n{}",
            raw.join("\n")
        );
    }
}

fn collect_rust_files(directory: &Path, output: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            if !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("target" | ".git")
            ) {
                collect_rust_files(&path, output);
            }
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            output.push(path);
        }
    }
}
