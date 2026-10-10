//! Checked catalogs and recursive backs of the installed browser profile.
use super::{linguistics, quantity_output};
use conduit_core::{CapabilityOffer, PortTemporal};
use std::cell::RefCell;
type InstalledCatalogs = (conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog);
thread_local! {
    // Exactly four immutable installed presentation profiles. Document-local
    // declarations/selectors mutate only independent copies returned below.
    static INSTALLED_CATALOGS: RefCell<Vec<(PresentationProfile, InstalledCatalogs)>> = const { RefCell::new(Vec::new()) };
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PresentationProfile {
    Annotation,
    Quantity,
    NormalizedDurations,
    PatternComparison,
}

pub(crate) fn catalogs(
) -> Result<(conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog), String> {
    catalogs_for_presentation(PresentationProfile::Annotation)
}

pub(crate) fn catalogs_for_presentation(
    presentation: PresentationProfile,
) -> Result<InstalledCatalogs, String> {
    if let Some(catalogs) = INSTALLED_CATALOGS.with(|cache| {
        cache
            .borrow()
            .iter()
            .find(|(profile, _)| *profile == presentation)
            .map(|(_, catalogs)| catalogs.clone())
    }) {
        return Ok(catalogs);
    }
    let catalogs = build_catalogs_for_presentation(presentation)?;
    INSTALLED_CATALOGS.with(|cache| cache.borrow_mut().push((presentation, catalogs.clone())));
    Ok(catalogs)
}

fn build_catalogs_for_presentation(
    presentation: PresentationProfile,
) -> Result<InstalledCatalogs, String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profile)?;
    conduit_speech::authoring::install(&mut startup)?;
    conduit_speech::ipa_constructors::install(&mut startup, &mut profile)?;
    conduit_speech::ipa_constructors::install_notation(&mut startup, &profile)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_typed_record_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_ordered_record_queue_catalog(&mut startup, &mut profile)?;
    conduit_net::install_record_temporal_catalogs(&mut startup, &mut profile)?;
    conduit_net::install_record_delivery_status_catalog(&mut startup, &mut profile)?;
    conduit_net::install_record_transcript_catalog(&mut startup, &mut profile)?;
    conduit_time::install_historical_timeline_catalog(&mut startup, &mut profile)?;
    conduit_time::install_replay_source_catalog(&mut startup, &mut profile)?;
    conduit_time::install_replay_control_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_window_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_summary_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_threshold_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_plot_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_plot_source_catalog(&mut startup, &mut profile)?;
    conduit_data::install_measurement_observation_catalog(&mut startup, &mut profile)?;
    conduit_little_seismograph_fixture::install_little_seismograph_fixture_catalog(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_signal_garden_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profile)?;
    match presentation {
        PresentationProfile::Annotation => {
            linguistics::install_catalogs(&mut startup, &mut profile)?
        }
        PresentationProfile::Quantity => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            quantity_output::install_catalogs(&mut startup, &mut profile)?;
        }
        PresentationProfile::PatternComparison => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            super::comparison_presentation::install_catalogs(&mut startup, &mut profile)?;
        }
        PresentationProfile::NormalizedDurations => {
            conduit_language::install_linguistics_catalogs(&mut startup, &mut profile)?;
            super::normalized_presentation::install_catalogs(&mut startup, &mut profile)?;
        }
    }
    conduit_semantic_catalog::install_value_primitive_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_body_startup_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_quantity_mapping_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_pitch_tone_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_quantity_info_catalog(&mut startup, &mut profile)?;
    conduit_plot::quantity_conversion::install(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_normalized_quantity_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)?;
    super::pointer_selector::install_types(&mut startup, &mut profile)?;
    conduit_presentation::install_bounded_stroke_capture_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profile)?;
    // Reviewed Plots are checked before realization eligibility is known. Keep
    // the canonical retained-State contract available to the checker even
    // though this browser profile does not currently offer its Back.
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_button_attempt_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_timed_pattern_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_pattern_comparison_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_sequence_normalization_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_template_storage_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_final_normalized_pattern_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profile)?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_pulse_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_rhythm_presentation_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_patchbay_presentation_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_button_indicator_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_application_catalogs(&mut startup, &mut profile)?;
    super::tutorial_presenter::install_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(
        &mut startup,
        &mut profile,
    )?;
    conduit_chat::install_browser_chat_catalogs(&mut startup, &mut profile)?;
    conduit_chat::install_body_chat_catalog(&mut startup, &mut profile)?;
    conduit_ai::install_llm_semantic_catalog(&mut startup, &mut profile)?;
    conduit_ai::install_model_text_catalog(&mut startup, &mut profile)?;
    conduit_ai::install_model_work_catalog(&mut startup, &mut profile)?;
    conduit_tongues::install_house_conversation_catalog(&mut startup, &mut profile)?;
    conduit_tongues::install_house_conversation_plot_catalog(&mut startup, &mut profile)?;
    conduit_tongues::install_speech_recognition_catalog(&mut startup, &mut profile)?;
    conduit_tongues::install_speech_catalogs(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_audio_capture_push_to_talk_catalog(
        &mut startup,
        &mut profile,
    )?;
    conduit_semantic_catalog::install_audio_tone_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_audio_continuous_tone_catalog(&mut startup, &mut profile)?;
    conduit_semantic_catalog::install_audio_gain_catalog(&mut startup, &mut profile)?;
    startup.insert_value_kind_alias(
        "PcmFrames",
        conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
    )?;
    startup.insert(conduit_plot::KindSignature {
        kind: conduit_semantic_catalog::BOOL_PRESENTATION_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    conduit_semantic_catalog::install_bool_presentation_catalog(&mut profile)?;
    if presentation == PresentationProfile::Quantity {
        startup.insert_value_kind_alias(
            "Scalar",
            conduit_core::kind_id(conduit_core::SCALAR_INFO_ID),
        )?;
        startup.insert_value_kind_alias(
            "Quantity",
            conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
        )?;
    }
    let startup = conduit_plot::checked_physical_catalog_for_document(
        &conduit_plot::parse_syntax_document(""),
        &startup,
    )
    .map_err(|error| format!("installed physical catalogue refused: {error:?}"))?;
    Ok((startup, profile))
}

pub(crate) fn install_checked_structured_selectors(
    checked: &conduit_plot::CheckedSyntaxDocument,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<Vec<CapabilityOffer>, String> {
    let mut offers = Vec::new();
    for selector in checked
        .plots
        .iter()
        .flat_map(|plot| &plot.cords)
        .flat_map(|cord| &cord.stages)
        .filter_map(|stage| match stage {
            conduit_plot::CheckedCordStage::StructuredSelector { selector, .. } => Some(selector),
            _ => None,
        })
    {
        for temporal in [
            PortTemporal::Value,
            PortTemporal::Flow { closes: true },
            PortTemporal::Flow { closes: false },
        ] {
            let definition = conduit_plot::structured_selector_definition(selector, temporal);
            if profile.get(&definition.kind_id).is_none() {
                profile
                    .insert(definition)
                    .map_err(|error| error.to_string())?;
            }
            let offer = super::structured_selector::offer(selector, temporal);
            if !offers
                .iter()
                .any(|current: &CapabilityOffer| current.kind_id == offer.kind_id)
            {
                offers.push(offer);
            }
        }
    }
    Ok(offers)
}

pub(crate) fn offers_for_expanded_pure_expressions(
    expanded: &conduit_plot::ExpandedCanonicalPlot,
) -> Result<Vec<CapabilityOffer>, String> {
    let mut offers = Vec::new();
    for gear in &expanded.gears {
        if !matches!(
            gear.kind_contract_revision.as_str(),
            conduit_plot::PURE_EXPRESSION_REVISION | conduit_plot::PURE_FILTER_REVISION
        ) {
            continue;
        }
        let program = super::pure_expression::program_from_configuration(&gear.configuration)?;
        let temporal = gear
            .inputs
            .first()
            .map(|port| port.temporal)
            .ok_or("expanded pure expression input is absent")?;
        let offer = if gear.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
            super::pure_expression::filter_offer(&program, temporal)?
        } else {
            super::pure_expression::offer(&program, temporal)?
        };
        if !offers
            .iter()
            .any(|current: &CapabilityOffer| current.kind_id == offer.kind_id)
        {
            offers.push(offer);
        }
    }
    Ok(offers)
}

pub(crate) fn offers_for_expanded_time_windows(
    expanded: &conduit_plot::ExpandedCanonicalPlot,
) -> Result<Vec<CapabilityOffer>, String> {
    expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::TIME_WINDOW_KIND)
        .map(super::time_window::offer_for_expanded)
        .collect()
}

pub(crate) fn offers_for_expanded_flow_collects(
    expanded: &conduit_plot::ExpandedCanonicalPlot,
) -> Result<Vec<CapabilityOffer>, String> {
    expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::FLOW_COLLECT_KIND)
        .map(super::flow_collect::offer_for_expanded)
        .collect()
}

pub(crate) fn offers_for_expanded_time_samples(
    expanded: &conduit_plot::ExpandedCanonicalPlot,
) -> Result<Vec<CapabilityOffer>, String> {
    expanded
        .gears
        .iter()
        .filter(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::TIME_SAMPLE_KIND)
        .map(super::time_sample::offer_for_expanded)
        .collect()
}

pub(crate) fn backs(
    startup: &conduit_plot::StartupCatalog,
    profile: &conduit_plot::ProfileCatalog,
) -> Result<conduit_plot::CanonicalBackCatalog, String> {
    let mut backs = conduit_plot::CanonicalBackCatalog::new();
    conduit_text::install_morse_backs(startup, profile, &mut backs)?;
    conduit_data::install_measurement_plot_source_back(startup, profile, &mut backs)?;
    conduit_semantic_catalog::install_signal_garden_backs(startup, profile, &mut backs)?;
    conduit_tongues::install_single_shot_streaming_recognition_back(startup, profile, &mut backs)?;
    Ok(backs)
}

#[cfg(test)]
mod conversation_tests {
    use super::*;

    #[test]
    fn browser_workspace_can_check_model_authoring_without_offering_training() {
        let (startup, profile) = catalogs().unwrap();
        let syntax = conduit_plot::parse_syntax_document(include_str!(
            "../../../../../plots/model-authoring/main.conduit"
        ));
        assert!(syntax.diagnostics.is_empty());
        let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
        let expanded = conduit_plot::expand_canonical_plot_for_authoring(
            &checked,
            "model-authoring",
            &profile,
        )
        .unwrap()
        .expanded;
        assert_eq!(
            expanded.gears[0].kind_id.as_str(),
            conduit_ai::MODEL_WORK_KIND
        );
        assert!(conduit_planner::default_expanded_placements(
            &expanded,
            &[crate::installed_browser::advertisement(
                "browser/model-catalog".into(),
                "boot/model-catalog".into(),
            )],
        )
        .is_err());
    }

    #[test]
    fn browser_workspace_can_check_and_expand_conversation_meaning_without_offering_it() {
        let (startup, profile) = catalogs().unwrap();
        for (source, entry) in [
            (
                include_str!("../../../../../plots/body-chat/main.conduit"),
                "body-chat",
            ),
            (
                include_str!("../../../../../plots/live-conversation/main.conduit"),
                "spoken-live-conversation",
            ),
        ] {
            let syntax = conduit_plot::parse_syntax_document(source);
            assert!(syntax.diagnostics.is_empty());
            let checked = conduit_plot::check_syntax_document(&syntax, &startup).unwrap();
            let expanded =
                conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &profile)
                    .unwrap()
                    .expanded;
            assert!(!expanded.gears.is_empty());
            assert!(conduit_planner::default_expanded_placements(
                &expanded,
                &[crate::installed_browser::advertisement(
                    "browser/conversation-catalog".into(),
                    "boot/conversation-catalog".into(),
                )],
            )
            .is_err());
        }
    }
}
