//! Immutable installed authoring catalogs and optional DSP construction.
use super::conversation_catalog;
use conduit_plot::{ProfileCatalog, StartupCatalog};

pub(crate) fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    shared_standard_catalogs().cloned()
}

// The executable owns these immutable definitions once. Borrowing them supplies
// no Ready offer, Source permission or workspace grant; Root must separately
// retain and fund this shared backing for every dependent operation.
pub(super) fn shared_standard_catalogs() -> Result<&'static (StartupCatalog, ProfileCatalog), String>
{
    static CATALOGS: std::sync::OnceLock<Result<(StartupCatalog, ProfileCatalog), String>> =
        std::sync::OnceLock::new();
    CATALOGS
        .get_or_init(|| prepare_catalogs(false))
        .as_ref()
        .map_err(Clone::clone)
}

pub(super) fn shared_dsp_catalogs() -> Result<&'static (StartupCatalog, ProfileCatalog), String> {
    static CATALOGS: std::sync::OnceLock<Result<(StartupCatalog, ProfileCatalog), String>> =
        std::sync::OnceLock::new();
    CATALOGS
        .get_or_init(|| prepare_catalogs(true))
        .as_ref()
        .map_err(Clone::clone)
}

fn prepare_catalogs(include_dsp: bool) -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = conduit_plot::checked_physical_catalog_for_document(
        &conduit_plot::parse_syntax_document(""),
        &conduit_signal::primary_signal_startup_catalog(),
    )
    .map_err(|error| format!("installed physical catalogue refused: {error:?}"))?;
    let mut profiles = conduit_signal::primary_signal_profile_catalog();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_body_startup_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_quantity_info_catalog(&mut startup, &mut profiles)?;
    if include_dsp {
        // Keep shipped Types before other catalogs, as in the original full
        // catalog. Extending an already prepared base would redeclare Types.
        conduit_semantic_catalog::install_dsp_source_catalogs(&mut startup, &mut profiles)?;
    }
    // This first Todo vertical has one exact authored initial Form and a leaf
    // combine Kind. Retained source uses the same catalog after owner restart.
    conduit_todo_plot::install_todo_catalogs(&mut startup, &mut profiles, "Groceries")?;
    conduit_thermostat_plot::install_catalogs(&mut startup, &mut profiles)?;
    conduit_face::install_mask_plot_value_aliases(&mut startup)?;
    patchbay_application::install_patchbay_interaction_catalog(&mut startup, &mut profiles)?;
    conduit_body_make::deployment_runtime_catalog::install_creche_catalogs(
        &mut startup,
        &mut profiles,
    )?;
    conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_text_state_catalogs(&mut startup, &mut profiles)?;
    conduit_data::install_data_text_catalogs(&mut startup, &mut profiles)?;
    conduit_data::install_measurement_window_catalog(&mut startup, &mut profiles)?;
    conduit_data::install_measurement_plot_catalog(&mut startup, &mut profiles)?;
    conduit_speech::kernel::install(&mut startup, &mut profiles)?;
    conduit_speech::authoring::install(&mut startup)?;
    conduit_speech::ipa_constructors::install(&mut startup, &mut profiles)?;
    conduit_speech::ipa_constructors::install_notation(&mut startup, &profiles)?;
    conduit_plot::quantity_conversion::install(&mut startup, &mut profiles)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_indicator_face_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_tick_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_ambient_display_types(&mut startup)?;
    conduit_semantic_catalog::install_ambient_clock_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_ambient_clock_face_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_ambient_clock_snapshot_face_kind(
        &mut startup,
        &mut profiles,
    )?;
    conduit_semantic_catalog::install_ambient_wallpaper_face_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_resident_wallpaper_kinds(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_ambient_home_observation_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_ambient_home_face_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_quantity_observation_face_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_reported_generation_face_kind(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_environment_recording_history_kind(
        &mut startup,
        &mut profiles,
    )?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_tick_face_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_pulse_face_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_rhythm_face_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_state_toggle_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_face_composition_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_graphics_face_catalog(&mut startup, &mut profiles)?;
    conduit_face::install_bitmap_face_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_http_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_recurrence_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_schedule_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_calendar_provider_catalogs(&mut startup, &mut profiles)?;
    conduit_face::install_geometry_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profiles)?;
    conduit_language::install_linguistics_catalogs(&mut startup, &mut profiles)?;
    conduit_data::install_tabular_catalogs(&mut startup, &mut profiles)?;
    conduit_finance::install_finance_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_job_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_print_catalogs(&mut startup, &mut profiles)?;
    conduit_net::install_application_network_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_navigation_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    conversation_catalog::install(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_education_catalogs(&mut startup, &mut profiles)?;
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profiles)?;
    conduit_alife::install_lenia_catalogs(&mut startup, &mut profiles)?;
    Ok((startup, profiles))
}
