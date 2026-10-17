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
        syntax: conduit_plot::parse_syntax_document_with_glyph_notations(source, &startup),
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
    let syntax = conduit_plot::parse_syntax_document_with_glyph_notations(&source, &startup);
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
        let checked = conduit_plot::check_syntax_document_with_literal_constructors(
            &self.syntax,
            &self.startup,
            &self.profiles,
        )
        .map_err(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))?;
        conduit_speech::ipa_constructors::validate_source(&self.syntax, &checked)
            .map_err(|diagnostic| diagnostic.to_string())?;
        conduit_plot::quantity_conversion::validate_source(&self.syntax, &checked).map_err(
            |diagnostic| {
                format!(
                    "CND-QTY-001 at {}:{}..{}:{}: {:?}",
                    diagnostic.span.line,
                    diagnostic.span.column,
                    diagnostic.span.end_line,
                    diagnostic.span.end_column,
                    diagnostic.refusal
                )
            },
        )?;
        Ok(checked)
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
        let Some(backs) = dsp_source_backs_for(&checked)? else {
            return conduit_plot::expand_canonical_plot_for_authoring(
                &checked,
                entry,
                &self.profiles,
            )
            .map_err(|diagnostic| diagnostic.to_string());
        };
        conduit_plot::expand_canonical_plot_for_authoring_with_backs(
            &checked,
            entry,
            &self.profiles,
            backs,
        )
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
        let dsp = dsp_source_backs_for(&checked)?;
        if recursive {
            let mut backs = dsp.cloned().unwrap_or_default();
            conduit_text::install_morse_backs(&self.startup, &self.profiles, &mut backs)?;
            return conduit_plot::expand_canonical_plot_with_backs(
                &checked,
                entry,
                &self.profiles,
                &backs,
            )
            .map_err(|diagnostic| diagnostic.to_string());
        }
        match dsp {
            Some(backs) => conduit_plot::expand_canonical_plot_with_backs(
                &checked,
                entry,
                &self.profiles,
                backs,
            ),
            None => conduit_plot::expand_canonical_plot(&checked, entry, &self.profiles),
        }
        .map_err(|diagnostic| diagnostic.to_string())
    }
}

// Resolve aliases through checked meaning before selecting optional Source Backs.
// Only immutable compiled definitions are cached; no Host offers or live state.
fn dsp_source_backs_for(
    checked: &conduit_plot::CheckedSyntaxDocument,
) -> Result<Option<&'static CanonicalBackCatalog>, String> {
    if !checked.plots.iter().any(|plot| {
        plot.gears
            .iter()
            .chain(plot.cords.iter().flat_map(|cord| &cord.stages).filter_map(
                |stage| match stage {
                    conduit_plot::CheckedCordStage::InlineGear(gear)
                    | conduit_plot::CheckedCordStage::RelationalGear { gear, .. } => Some(gear),
                    _ => None,
                },
            ))
            .any(|gear| gear.kind.starts_with("audio/dsp/"))
    }) {
        return Ok(None);
    }
    static BACKS: std::sync::OnceLock<Result<CanonicalBackCatalog, String>> =
        std::sync::OnceLock::new();
    match BACKS.get_or_init(|| {
        let (_, profiles) = standard_catalogs()?;
        let mut backs = CanonicalBackCatalog::new();
        conduit_semantic_catalog::install_dsp_source_backs(&profiles, &mut backs)?;
        Ok(backs)
    }) {
        Ok(backs) => Ok(Some(backs)),
        Err(error) => Err(error.clone()),
    }
}

fn standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    // These catalogs contain only this executable's compiled semantic contracts.
    // Check them once, then give each Source its own mutable copy. Host offers,
    // authority and each authored document are still admitted independently.
    static CATALOGS: std::sync::OnceLock<Result<(StartupCatalog, ProfileCatalog), String>> =
        std::sync::OnceLock::new();
    CATALOGS.get_or_init(prepare_standard_catalogs).clone()
}

fn prepare_standard_catalogs() -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = conduit_plot::checked_physical_catalog_for_document(
        &conduit_plot::parse_syntax_document(""),
        &conduit_signal::primary_signal_startup_catalog(),
    )
    .map_err(|error| format!("installed physical catalogue refused: {error:?}"))?;
    let mut profiles = conduit_signal::primary_signal_profile_catalog();
    conduit_semantic_catalog::install_numerical_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_dsp_source_catalogs(&mut startup, &mut profiles)?;
    // This first Todo vertical has one exact authored initial Form and a leaf
    // combine Kind. Retained source uses the same catalog after owner restart.
    conduit_todo_plot::install_todo_catalogs(&mut startup, &mut profiles, "Groceries")?;
    conduit_thermostat_plot::install_catalogs(&mut startup, &mut profiles)?;
    conduit_face::install_mask_plot_value_aliases(&mut startup)?;
    conduit_face::install_mask_mechanism_catalog(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles)?;
    conduit_speech::kernel::install(&mut startup, &mut profiles)?;
    conduit_speech::authoring::install(&mut startup)?;
    conduit_speech::ipa_constructors::install(&mut startup, &mut profiles)?;
    conduit_speech::ipa_constructors::install_notation(&mut startup, &profiles)?;
    conduit_plot::quantity_conversion::install(&mut startup, &mut profiles)?;
    conduit_text::install_morse_catalogs(&mut startup, &mut profiles)?;
    conduit_semantic_catalog::install_indicator_face_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_tick_catalog(&mut startup, &mut profiles)?;
    conduit_time::install_time_every_catalog(&mut startup, &mut profiles)?;
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
    fn product_authoring_expands_the_owned_public_dsp_back() {
        let original = include_str!("../../../semantics/audio/dsp/public-gain.conduit");
        let alias = format!(
            "with audio/dsp/gain as ApplyGain\n{}",
            original.replace("audio/dsp/gain()", "ApplyGain()")
        );
        for text in [original, alias.as_str()] {
            let source = parse(text).unwrap();
            let checked = source.check().unwrap();
            let first = dsp_source_backs_for(&checked).unwrap().unwrap();
            let second = dsp_source_backs_for(&checked).unwrap().unwrap();
            assert!(std::ptr::eq(first, second));
            let expanded = source.expand_entry_for_authoring().unwrap();
            assert_eq!(expanded.expanded.name, "caller");
            assert!(!expanded.expanded.gears.is_empty());
        }
    }

    #[test]
    fn inline_dsp_selection_preserves_the_exact_port_contract() {
        for (kind, import) in [
            ("audio/dsp/gain", ""),
            ("ApplyGain", "with audio/dsp/gain as ApplyGain\n"),
        ] {
            let text = format!("with audio/dsp/GainControl as Input\nwith audio/dsp/GainOutcome as Output\n{import}plot caller (\n >> value: Input...|\n result: Output...| >>\n) {{\n value >> {kind}() >> result\n}}\n");
            let source = parse(&text).unwrap();
            assert!(dsp_source_backs_for(&source.check().unwrap())
                .unwrap()
                .is_some());
            // This Fore deliberately has named ports and no implicit shorthand.
            let Err(error) = source.expand_entry_for_authoring() else {
                panic!("DSP ports cannot be invented by inline selection")
            };
            assert!(error.contains("CND-FRM-044"), "{error}");
        }
    }

    #[test]
    fn ordinary_todo_source_expands_without_preparing_unused_dsp_owners() {
        let source = parse(include_str!("../../../plots/todo/live.conduit")).unwrap();
        assert!(dsp_source_backs_for(&source.check().unwrap())
            .unwrap()
            .is_none());
        assert_eq!(
            source.expand_entry_for_authoring().unwrap().expanded.name,
            "todo/main"
        );
    }

    #[test]
    fn prepared_standard_catalogs_do_not_share_source_local_types() {
        let (mut first, first_profiles) = standard_catalogs().unwrap();
        let (second, second_profiles) = standard_catalogs().unwrap();
        assert_eq!(first, second);
        assert_eq!(first_profiles, second_profiles);
        let local = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document("type QueueLocal = Text <= 8B\n"),
            &StartupCatalog::new(),
        )
        .unwrap();
        first
            .insert_checked_native_type("QueueLocal", &local.native_types[0])
            .unwrap();
        assert_ne!(first, second);
        assert_eq!(second, standard_catalogs().unwrap().0);
    }

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
    fn product_authoring_checks_quoted_ipa_with_the_ordinary_speech_catalog() {
        let source = parse(include_str!(
            "../../../semantics/speech/examples/ipa/quoted-phone.conduit"
        ))
        .unwrap();
        assert_eq!(
            source.expand_entry_for_authoring().unwrap().expanded.name,
            "quoted-phone"
        );
        let invalid = parse(&source.source.replace("tʰ", "p_aspirated")).unwrap();
        assert!(invalid.expand_entry_for_authoring().is_err());
    }

    #[test]
    fn product_checks_qualified_quoted_transcriptions_and_located_refusals() {
        for text in [
            include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit"),
            include_str!("../../../semantics/speech/examples/ipa/quoted-phonemic.conduit"),
        ] {
            let source = parse(text).unwrap();
            assert!(source.expand_entry_for_authoring().is_ok());
        }
        let invalid = parse(
            &include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit")
                .replace("ˈt͡ʃãː.n̩", r"t͡ʃ\n"),
        )
        .unwrap();
        let error = invalid.check().unwrap_err();
        assert!(error.contains("CND-SPC-IPA at 5:"), "{error}");
    }

    #[test]
    fn product_compiler_checks_the_ordinary_mask_plot_boundary() {
        let source = parse(
            "plot browser-mask (\n    >> face: Face\n    interaction: FaceInteraction...| >>\n    show: Show >>\n) {\n}\n",
        )
        .unwrap();
        let checked = conduit_plot::check_syntax_document(&source.syntax, &source.startup).unwrap();
        let front = &checked.plots[0].runtime_front;

        assert_eq!(
            front.inputs()[0].value_kind.as_str(),
            conduit_face::FACE_VALUE_KIND
        );
        assert_eq!(
            front.outputs()[0].value_kind.as_str(),
            conduit_face::FACE_INTERACTION_VALUE_KIND
        );
        assert_eq!(
            front.outputs()[1].value_kind.as_str(),
            conduit_face::SHOW_VALUE_KIND
        );
    }
}
