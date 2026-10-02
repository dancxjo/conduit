use conduit_core::{kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal};
use conduit_plot::{KindProjection, ProfileCatalog};
use serde::Serialize;

use super::super::CatalogError;

const TONE_SOURCE: &str = "conduit-conformance/tone-source";
const NOTE_SOURCE: &str = "conduit-conformance/note-source";
const CONTROL_SOURCE: &str = "conduit-conformance/control-source";
const RENDER_SOURCE: &str = "conduit-conformance/render-source";

const TONE_FORM: &str = "plot tone {\n source: conduit-conformance/tone-source\n output: sound/tone-play\n source.tone >> output.tone\n}\n";
pub(super) const SIMPLE_FORM: &str = conduit_pete::SIMPLE_MELODY_FORM;
const EXPRESSIVE_FORM: &str = "plot expressive-synthesis {\n notes: conduit-conformance/note-source\n controls: conduit-conformance/control-source\n render: conduit-conformance/render-source\n synth: music/synth(maximum-voices = 8, oscillator = \"saw\")\n output: audio/play\n notes.notes >> synth.notes\n controls.controls >> synth.controls\n render.render >> synth.render\n synth.audio >> output.audio\n}\n";

#[derive(Debug, Serialize)]
pub(super) struct CanonicalPlot {
    id: &'static str,
    required_profiles: &'static [&'static str],
    source: &'static str,
    source_document_id: String,
    checked_plot_id: String,
    expanded_plot_id: String,
}

pub(super) fn build() -> Result<Vec<CanonicalPlot>, CatalogError> {
    let catalog = catalog()?;
    [
        ("tone-a", &["tone"][..], TONE_FORM),
        (
            "simple-music-b",
            &["simple-monophonic-notes", "simple-polyphonic-notes"][..],
            SIMPLE_FORM,
        ),
        (
            "expressive-synth-c",
            &[
                "expressive-notes",
                "expressive-subtractive-synthesis",
                "pcm-s16le-48000-stereo-p256",
            ][..],
            EXPRESSIVE_FORM,
        ),
    ]
    .into_iter()
    .map(|(id, required_profiles, source)| checked(&catalog, id, required_profiles, source))
    .collect()
}

fn checked(
    catalog: &ProfileCatalog,
    id: &'static str,
    required_profiles: &'static [&'static str],
    source: &'static str,
) -> Result<CanonicalPlot, CatalogError> {
    for forbidden in [
        "midi",
        "alsa",
        "opl",
        "create",
        "pc-speaker",
        "device",
        "backend",
        "serial",
    ] {
        if source.to_ascii_lowercase().contains(forbidden) {
            return Err(CatalogError::new(
                "sound-plot-contains-mechanism",
                format!("canonical Plot {id} contains {forbidden}"),
            ));
        }
    }
    let plot = conduit_plot::parse(source, catalog).map_err(|error| {
        CatalogError::new(
            "sound-plot-invalid",
            format!("canonical Plot {id} did not check: {error}"),
        )
    })?;
    Ok(CanonicalPlot {
        id,
        required_profiles,
        source,
        source_document_id: plot.source_document_id.as_str().to_owned(),
        checked_plot_id: plot.checked_plot_id.as_str().to_owned(),
        expanded_plot_id: plot.expanded_plot_id.as_str().to_owned(),
    })
}

fn catalog() -> Result<ProfileCatalog, CatalogError> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut catalog = ProfileCatalog::new();
    conduit_semantic_catalog::install_sound_catalogs(&mut startup, &mut catalog).map_err(
        |error| {
            CatalogError::new(
                "sound-catalog-invalid",
                format!("standard sound catalog failed: {error:?}"),
            )
        },
    )?;
    for (kind, port, info) in [
        (TONE_SOURCE, "tone", conduit_audio::SOUND_TONE_INFO_ID),
        (NOTE_SOURCE, "notes", conduit_audio::MUSIC_NOTE_INFO_ID),
        (
            CONTROL_SOURCE,
            "controls",
            conduit_audio::MUSIC_CONTROL_INFO_ID,
        ),
        (
            RENDER_SOURCE,
            "render",
            conduit_audio::AUDIO_RENDER_DEMAND_INFO_ID,
        ),
    ] {
        catalog
            .insert(source(kind, port, info))
            .map_err(|error| CatalogError::new("sound-source-kind-invalid", error.to_string()))?;
    }
    Ok(catalog)
}

fn source(kind: &str, port: &str, info: &str) -> KindProjection {
    KindProjection {
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(format!("{kind}@1")),
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id(port),
            value_kind: kind_id(info),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_plots_check_and_contain_no_realization_names() {
        let plots = build().unwrap();
        assert_eq!(plots.len(), 3);
        assert!(plots.iter().all(|plot| {
            !plot.source_document_id.is_empty()
                && !plot.checked_plot_id.is_empty()
                && !plot.expanded_plot_id.is_empty()
        }));
        assert_ne!(plots[0].source_document_id, plots[1].source_document_id);
        assert_ne!(plots[1].source_document_id, plots[2].source_document_id);
    }
}
