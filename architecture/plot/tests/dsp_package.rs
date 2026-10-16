//! Prototype the existing checked-package seam with unchanged DSP owners.
use conduit_plot::*;
const MANIFEST: &str = "pack audio/dsp (\n version = 1.0.0\n) {\n ship noise-step-candidate\n ship NoiseStepCandidate\n ship NoiseStepOutcome\n ship envelope\n ship EnvelopeControl\n ship EnvelopeOutcome\n ship gain\n ship GainControl\n ship GainOutcome\n ship one-pole\n ship OnePoleFrame\n ship OnePoleOutcome\n}\n";
fn members() -> [PackageMemberSource<'static>; 4] {
    [
        PackageMemberSource {
            path: "noise",
            source: include_str!("../../../semantics/audio/dsp/seeded-noise.conduit"),
        },
        PackageMemberSource {
            path: "envelope",
            source: include_str!("../../../semantics/audio/dsp/envelope.conduit"),
        },
        PackageMemberSource {
            path: "gain",
            source: include_str!("../../../semantics/audio/dsp/gain.conduit"),
        },
        PackageMemberSource {
            path: "one-pole",
            source: include_str!("../../../semantics/audio/dsp/one-pole.conduit"),
        },
    ]
}
#[test]
fn checked_pack_exports_real_dsp_types_and_owned_expression_programs() {
    let syntax = parse_syntax_document(MANIFEST);
    let members = members();
    let bundle =
        CheckedPackageBundle::from_sources(MANIFEST, &syntax.packages[0], &members).unwrap();
    let checked = check_package_bundle(
        &bundle,
        MANIFEST,
        &syntax.packages[0],
        &members,
        &StartupCatalog::new(),
    )
    .unwrap();
    let exports =
        PackageExportCatalog::from_bundle(&bundle, MANIFEST, &syntax.packages[0], &members)
            .unwrap();
    for leaf in ["noise-step-candidate", "envelope", "gain", "one-pole"] {
        let exported = exports.resolve(&format!("audio/dsp/{leaf}")).unwrap();
        let authored = expand_canonical_plot_for_authoring(
            &checked,
            &exported.name.text,
            &ProfileCatalog::new(),
        )
        .unwrap();
        assert!(!authored.expanded.gears.is_empty());
    }
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    let downstream=check_syntax_document(&parse_syntax_document("with audio/dsp/GainControl as GainInput\nplot use-gain-type (\n >> value: GainInput\n) {\n}\n"),&catalog).unwrap();
    let owner = checked.plots.iter().find(|p| p.name == "dsp/gain").unwrap();
    assert_eq!(
        downstream.plots[0].checked_front().inputs()[0].value_kind,
        owner.checked_front().inputs()[0].value_kind
    );
    let mut changed = members.clone();
    changed[0].source = "plot counterfeit {\n}\n";
    assert!(
        PackageExportCatalog::from_bundle(&bundle, MANIFEST, &syntax.packages[0], &changed)
            .is_err()
    );
}
