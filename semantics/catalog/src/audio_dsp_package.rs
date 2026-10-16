//! Exact checked Source-package preparation, with no Host offer or execution.
use alloc::{format, string::String};
use conduit_plot::{
    check_package_bundle, parse_syntax_document, prepare_source_types, CheckedPackageBundle,
    CheckedSyntaxDocument, PackageExportCatalog, PackageMemberSource, StartupCatalog,
};

#[path = "audio_dsp_profiles.rs"]
mod profiles;

pub const DSP_PACKAGE_MANIFEST: &str = include_str!("../../audio/dsp/pack.conduit");

/// Source and export facts derived by the ordinary checked-package owners.
/// Preparation may allocate; this object neither chooses Backs nor runs Play.
pub struct DspSourcePackage {
    pub bundle: CheckedPackageBundle,
    pub checked: CheckedSyntaxDocument,
    pub exports: PackageExportCatalog,
    /// Exact structural tuple aliases needed by the authored mixer Fores.
    pub startup: StartupCatalog,
}

/// Rebuilds the package from immutable owned Source. The compound oscillator
/// and resonator members retain their existing complete declaration groups;
/// component bodies and equations are never rewritten by this preparation.
pub fn prepare_dsp_source_package() -> Result<DspSourcePackage, String> {
    let types = [
        include_str!("../../audio/types.conduit"),
        include_str!("../../audio/trajectory_types.conduit"),
        include_str!("../../audio/rate_types.conduit"),
        include_str!("../../audio/decibel_types.conduit"),
        include_str!("../../audio/exact_control_types.conduit"),
        include_str!("../../audio/time_analysis_types.conduit"),
        include_str!("../../audio/spectral_analysis_types.conduit"),
    ]
    .join("\n");
    let oscillator = [
        include_str!("../../audio/dsp/periodic-types.conduit"),
        conduit_audio::PERIODIC_VALIDATION_SOURCE,
        conduit_audio::PERIODIC_FLOW_SOURCE,
        conduit_audio::PERIODIC_OSCILLATOR_SOURCE,
    ]
    .join("\n");
    let resonator = [
        conduit_audio::RESONATOR_RECURRENCE_SOURCE,
        include_str!("../../audio/dsp/resonator-types.conduit"),
        include_str!("../../audio/dsp/resonator-projection.conduit"),
        include_str!("../../audio/dsp/resonator.conduit"),
    ]
    .join("\n");
    let noise_owner = include_str!("../../audio/dsp/seeded-noise.conduit");
    let noise = format!(
        "{noise_owner}\n{}",
        profiles::value_profile(
            noise_owner,
            "dsp/noise-step-candidate",
            "dsp/noise-step-value"
        )?
    );
    let envelope_owner = include_str!("../../audio/dsp/envelope.conduit");
    let envelope = format!(
        "{envelope_owner}\n{}",
        profiles::value_profile(envelope_owner, "dsp/envelope", "dsp/envelope-value")?
    );
    let gain_owner = include_str!("../../audio/dsp/gain.conduit");
    let gain = format!(
        "{gain_owner}\n{}",
        profiles::value_profile(gain_owner, "dsp/gain", "dsp/gain-value")?
    );
    let mixer_owner = include_str!("../../audio/dsp/mixer.conduit");
    let mixer = format!(
        "{mixer_owner}\n{}",
        profiles::value_profile(mixer_owner, "dsp/mix-two", "dsp/mix-two-value")?
    );
    let mut startup = StartupCatalog::new();
    let mixer_types = prepare_source_types(&parse_syntax_document(&mixer), &startup)
        .map_err(|e| format!("DSP mixer Types: {e:?}"))?;
    let channel = mixer_types
        .named_type("MixerChannel")
        .ok_or_else(|| String::from("DSP mixer lacks its owned channel Type"))?
        .clone();
    let pair = conduit_core::tuple_info_type(alloc::vec![channel.clone(), channel])
        .map_err(|e| format!("DSP mixer pair: {e:?}"))?;
    let quad = conduit_core::tuple_info_type(alloc::vec![pair.clone(), pair.clone()])
        .map_err(|e| format!("DSP mixer quad: {e:?}"))?;
    startup
        .insert_structured_type("MixerPair", pair)
        .map_err(|e| format!("DSP mixer pair catalog: {e:?}"))?;
    startup
        .insert_structured_type("MixerQuad", quad)
        .map_err(|e| format!("DSP mixer quad catalog: {e:?}"))?;
    let members = [
        PackageMemberSource {
            path: "types",
            source: &types,
        },
        PackageMemberSource {
            path: "oscillator",
            source: &oscillator,
        },
        PackageMemberSource {
            path: "resonator",
            source: &resonator,
        },
        PackageMemberSource {
            path: "mixer",
            source: &mixer,
        },
        PackageMemberSource {
            path: "noise",
            source: &noise,
        },
        PackageMemberSource {
            path: "envelope",
            source: &envelope,
        },
        PackageMemberSource {
            path: "gain",
            source: &gain,
        },
        PackageMemberSource {
            path: "example",
            source: include_str!("../../audio/dsp/noise-gain-example.conduit"),
        },
        PackageMemberSource {
            path: "one-pole",
            source: include_str!("../../audio/dsp/one-pole.conduit"),
        },
    ];
    let document = parse_syntax_document(DSP_PACKAGE_MANIFEST);
    let manifest = document
        .packages
        .first()
        .ok_or_else(|| String::from("DSP package manifest missing"))?;
    let bundle = CheckedPackageBundle::from_sources(DSP_PACKAGE_MANIFEST, manifest, &members)
        .map_err(|e| format!("DSP package bundle: {e:?}"))?;
    let checked = check_package_bundle(&bundle, DSP_PACKAGE_MANIFEST, manifest, &members, &startup)
        .map_err(|e| format!("DSP package check: {e:?}"))?;
    let exports =
        PackageExportCatalog::from_bundle(&bundle, DSP_PACKAGE_MANIFEST, manifest, &members)
            .map_err(|e| format!("DSP package exports: {e:?}"))?;
    Ok(DspSourcePackage {
        bundle,
        checked,
        exports,
        startup,
    })
}
