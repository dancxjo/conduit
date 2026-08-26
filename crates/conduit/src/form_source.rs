//! The canonical product source-loading boundary.

use conduit_form::{ExpandedCanonicalForm, ProfileCatalog, StartupCatalog};
use std::fs;
use std::path::Path;

pub(crate) struct CanonicalSource {
    pub(crate) source: String,
    pub(crate) syntax: conduit_form::SyntaxDocument,
    pub(crate) startup: StartupCatalog,
    profiles: ProfileCatalog,
}

pub(crate) fn load(path: &Path) -> Result<CanonicalSource, String> {
    let (startup, profiles) = standard_catalogs()?;
    load_with_catalogs(path, startup, profiles)
}

pub(crate) fn load_signal(path: &Path) -> Result<CanonicalSource, String> {
    load_with_catalogs(
        path,
        conduit_signal::signal_startup_catalog(),
        conduit_signal::signal_profile_catalog(),
    )
}

fn load_with_catalogs(
    path: &Path,
    startup: StartupCatalog,
    profiles: ProfileCatalog,
) -> Result<CanonicalSource, String> {
    if path.extension().and_then(std::ffi::OsStr::to_str) != Some("conduit") {
        return Err(format!(
            "canonical Form source must use the .conduit suffix: {}",
            path.display()
        ));
    }
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let syntax = conduit_form::parse_syntax_document(&source);
    Ok(CanonicalSource {
        source,
        syntax,
        startup,
        profiles,
    })
}

impl CanonicalSource {
    pub(crate) fn expand_entry(&self) -> Result<ExpandedCanonicalForm, String> {
        if let Some(diagnostic) = self.syntax.diagnostics.first() {
            return Err(format!("{}: {}", diagnostic.code, diagnostic.message));
        }
        let checked = conduit_form::check_syntax_document(&self.syntax, &self.startup)
            .map_err(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))?;
        let entry = checked
            .forms
            .last()
            .ok_or_else(|| "canonical Form source contains no Form".to_string())?
            .name
            .clone();
        conduit_form::expand_canonical_form(&checked, &entry, &self.profiles)
            .map_err(|diagnostic| diagnostic.to_string())
    }
}

fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    conduit_std_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_time_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_timing_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_count_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_flow_state_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_state_toggle_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_logic_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_math_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_layout_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_presentation_composition_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_graphics_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_graphics_presentation_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_keyboard_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_input_semantic_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_http_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_recurrence_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_schedule_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_calendar_provider_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_geometry_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_vision_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_linguistics_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_tabular_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_finance_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_job_catalogs(&mut startup, &mut profiles)?;
    conduit_net::install_application_network_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_robotics_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_robotics_structured_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_education_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_messaging_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_generalized_input_catalogs(&mut startup, &mut profiles)?;
    conduit_std_catalog::install_alife_catalogs(&mut startup, &mut profiles)?;
    Ok((startup, profiles))
}
