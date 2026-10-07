#![cfg(feature = "kernel")]
use conduit_ai::native_profile::PreparedNativeProfile;
use conduit_plot::maximum_prepared_canonical_value_bytes;
fn declarations_only(source: &str) -> String {
    let mut active = false;
    let mut result = String::new();
    for line in source.lines() {
        if line.starts_with("type ") {
            active = true;
        }
        if line.starts_with("plot ") {
            active = false;
        }
        if active {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
fn exact_epoch_declarations() -> String {
    [
        include_str!("../../ai/fixed_numeric.conduit"),
        include_str!("../../ai/fixed_numeric_signal.conduit"),
        include_str!("../fargan_conditioning.conduit"),
        include_str!("../fargan_signal.conduit"),
        include_str!("../fargan_subframe.conduit"),
        include_str!("../fargan_model_identity.conduit"),
        include_str!("../fargan_epoch_contracts.conduit"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, source)| {
        if index < 2 {
            source
                .lines()
                .filter(|line| {
                    line.starts_with("type NumericFiniteF32 ")
                        || line.starts_with("type NumericF32Vector")
                        || line.starts_with("type NumericI16Vector")
                        || line.starts_with("type NumericHistory")
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            declarations_only(source)
        }
    })
    .collect::<Vec<_>>()
    .join("\n")
}
#[test]
fn exact_source_native_epoch_profiles_and_distinct_candidates_fit_selected_frame() {
    let declarations = exact_epoch_declarations();
    for (name, expected) in [
        ("FarganFloatEpochInput", 14255),
        ("FarganFloatPhase1", 14138),
        ("FarganFloatPhase2", 13778),
        ("FarganFloatPhase3", 13417),
        ("FarganFloatEpochProposal", 12672),
        ("FarganPcm16EpochResult", 12246),
    ] {
        let profile = PreparedNativeProfile::check_definition(&declarations, name).unwrap();
        let native = maximum_prepared_canonical_value_bytes(profile.value_type()).unwrap();
        let candidate = maximum_prepared_canonical_value_bytes(profile.candidate_type()).unwrap();
        assert_eq!(native, expected);
        assert!(native <= 16384 && candidate <= 16384);
        assert_ne!(profile.value_type(), profile.candidate_type());
        assert_eq!(
            profile.definition_digest(),
            PreparedNativeProfile::check_definition(&declarations, name)
                .unwrap()
                .definition_digest()
        );
        eprintln!("{name}: exact native={native}, raw candidate={candidate}, selected frame=16384");
        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profiles = conduit_plot::ProfileCatalog::new();
        profile.install(&mut startup, &mut profiles, true).unwrap();
    }
}
