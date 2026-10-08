//! The canonical product source-loading boundary.

use conduit_plot::{CanonicalBackCatalog, ExpandedCanonicalPlot, ProfileCatalog, StartupCatalog};
use std::fs;
use std::path::Path;

pub(crate) struct CanonicalSource {
    pub(crate) source: String,
    pub(crate) syntax: conduit_plot::SyntaxDocument,
    pub(crate) startup: StartupCatalog,
    profiles: ProfileCatalog,
}

pub(crate) fn load(path: &Path) -> Result<CanonicalSource, String> {
    let (startup, profiles) = standard_catalogs()?;
    load_with_catalogs(path, startup, profiles)
}

// Used by the library entrance; the binary compiles this module independently.
#[allow(dead_code)]
pub(crate) fn parse(source: &str) -> Result<CanonicalSource, String> {
    let (startup, profiles) = standard_catalogs()?;
    Ok(CanonicalSource {
        source: source.into(),
        syntax: conduit_plot::parse_syntax_document(source),
        startup,
        profiles,
    })
}

fn load_with_catalogs(
    path: &Path,
    startup: StartupCatalog,
    profiles: ProfileCatalog,
) -> Result<CanonicalSource, String> {
    if path.extension().and_then(std::ffi::OsStr::to_str) != Some("conduit") {
        return Err(format!(
            "canonical Plot source must use the .conduit suffix: {}",
            path.display()
        ));
    }
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let syntax = conduit_plot::parse_syntax_document(&source);
    Ok(CanonicalSource {
        source,
        syntax,
        startup,
        profiles,
    })
}

impl CanonicalSource {
    /// Checked Plots may be reordered for dependency lowering. The public
    /// entry is the final top-level Plot the author declared, not the final
    /// dependency in the checked catalogue.
    fn authored_entry_name(&self) -> Result<&str, String> {
        self.syntax
            .plots
            .last()
            .map(|plot| plot.name.text.as_str())
            .ok_or_else(|| "canonical Plot source contains no Plot".to_string())
    }

    // The library entrance compiles this source loader independently of the
    // installed owner binary.
    #[allow(dead_code)]
    pub(crate) fn authoring_catalog(&self) -> &ProfileCatalog {
        &self.profiles
    }

    pub(crate) fn check(&self) -> Result<conduit_plot::CheckedSyntaxDocument, String> {
        if let Some(diagnostic) = self.syntax.diagnostics.first() {
            return Err(format!("{}: {}", diagnostic.code, diagnostic.message));
        }
        conduit_plot::check_syntax_document(&self.syntax, &self.startup)
            .map_err(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
    }

    pub(crate) fn expand_entry(&self) -> Result<ExpandedCanonicalPlot, String> {
        self.expand_entry_with_backs(false)
    }

    // Used by the binary diagram entrance; the library compiles this module independently.
    #[allow(dead_code)]
    pub(crate) fn expand_entry_for_authoring(
        &self,
    ) -> Result<conduit_plot::ExpandedAuthoringPlot, String> {
        let checked = self.check()?;
        let entry = self.authored_entry_name()?;
        conduit_plot::expand_canonical_plot_for_authoring(&checked, entry, &self.profiles)
            .map_err(|diagnostic| diagnostic.to_string())
    }

    // Used by the library entrance; the binary compiles this module independently.
    #[allow(dead_code)]
    pub(crate) fn expand_entry_recursive(&self) -> Result<ExpandedCanonicalPlot, String> {
        self.expand_entry_with_backs(true)
    }

    fn expand_entry_with_backs(&self, recursive: bool) -> Result<ExpandedCanonicalPlot, String> {
        let checked = self.check()?;
        let entry = self.authored_entry_name()?;
        if recursive {
            let mut backs = CanonicalBackCatalog::new();
            conduit_text::install_morse_backs(&self.startup, &self.profiles, &mut backs)?;
            conduit_plot::expand_canonical_plot_with_backs(&checked, entry, &self.profiles, &backs)
                .map_err(|diagnostic| diagnostic.to_string())
        } else {
            conduit_plot::expand_canonical_plot(&checked, entry, &self.profiles)
                .map_err(|diagnostic| diagnostic.to_string())
        }
    }
}

fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = conduit_signal::primary_signal_startup_catalog();
    let mut profiles = conduit_signal::primary_signal_profile_catalog();
    // This first Todo vertical has one exact authored initial Form and a leaf
    // combine Kind. Retained source uses the same catalog after owner restart.
    conduit_todo_plot::install_todo_catalogs(&mut startup, &mut profiles, "Groceries")?;
    conduit_std_host::install_ipa_catalog(&mut startup, &mut profiles)?;
    conduit_presentation::install_mask_plot_value_aliases(&mut startup)?;
    conduit_presentation::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_speech::kernel::install(&mut startup, &mut profiles)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_indicator_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_tick_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_rhythm_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_tick_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_pulse_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_rhythm_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_timing_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_count_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_flow_state_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_state_toggle_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_logic_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_math_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_layout_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_presentation_composition_catalogs(
        &mut startup,
        &mut profiles,
    )?;
    conduit_semantic_catalog::install_graphics_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_graphics_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_presentation::install_bitmap_presentation_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_keyboard_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_input_semantic_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_http_catalogs(&mut startup, &mut profiles)?;
    conduit_web::install_json_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_recurrence_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_schedule_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_calendar_provider_catalogs(&mut startup, &mut profiles)?;
    conduit_presentation::install_geometry_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_vision_catalogs(&mut startup, &mut profiles)?;
    conduit_language::install_linguistics_catalogs(&mut startup, &mut profiles)?;
    conduit_data::install_tabular_catalogs(&mut startup, &mut profiles)?;
    conduit_finance::install_finance_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_job_catalogs(&mut startup, &mut profiles)?;
    conduit_net::install_application_network_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_robotics_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_robotics_structured_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_navigation_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_education_catalogs(&mut startup, &mut profiles)?;
    conduit_chat::install_messaging_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_generalized_input_catalogs(&mut startup, &mut profiles)?;
    conduit_alife::install_lenia_catalogs(&mut startup, &mut profiles)?;
    Ok((startup, profiles))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_entry_survives_checked_dependency_reordering() {
        let source = parse(include_str!("../../../plots/todo/live.conduit")).unwrap();
        assert_eq!(source.syntax.plots.last().unwrap().name.text, "todo/main");
        assert_eq!(
            source.check().unwrap().plots.last().unwrap().name,
            "todo/transition"
        );
        assert_eq!(
            source.expand_entry_for_authoring().unwrap().expanded.name,
            "todo/main"
        );
    }

    #[test]
    fn product_compiler_checks_the_ordinary_mask_plot_boundary() {
        let source = parse(
            "plot browser-mask (\n    >> face: Presentation\n    interaction: FaceInteraction...| >>\n    show: Show >>\n) {\n}\n",
        )
        .unwrap();
        let checked = conduit_plot::check_syntax_document(&source.syntax, &source.startup).unwrap();
        let front = &checked.plots[0].runtime_front;

        assert_eq!(
            front.inputs()[0].value_kind.as_str(),
            conduit_presentation::PRESENTATION_VALUE_KIND
        );
        assert_eq!(
            front.outputs()[0].value_kind.as_str(),
            conduit_presentation::FACE_INTERACTION_VALUE_KIND
        );
        assert_eq!(
            front.outputs()[1].value_kind.as_str(),
            conduit_presentation::SHOW_VALUE_KIND
        );
    }
}
