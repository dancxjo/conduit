//! Source startup policy operates on three separately bounded admitted domains.
use super::*;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn prepare_startup_profiles() -> (
    EpochProfiles,
    BTreeMap<String, String>,
    Vec<CapabilityOffer>,
) {
    let mut context = prepared_epoch_profiles_with_capacity(true);
    let definition = startup_definition();
    let mut ids = BTreeMap::new();
    let mut offers = vec![];
    for (key, material, output) in [
        (
            "SIGNAL",
            "FarganNativeSignalStartup",
            "FarganSignalEpochFeedback",
        ),
        (
            "CONDITIONING",
            "FarganNativeConditioningStartup",
            "FarganConditioningEpochFeedback",
        ),
        (
            "FEATURE",
            "FarganFeatureProposalEpoch",
            "FarganFeatureEpochFeedback",
        ),
    ] {
        let input =
            Arc::new(PreparedNativeProfile::check_definition(&definition, material).unwrap());
        let weak = Arc::new(PreparedNominalWeakening::prepare(input.value_type().clone()).unwrap());
        weak.install(&mut context.startup, &mut context.profiles, false)
            .unwrap();
        offers.push(weak.offer(false).unwrap());
        ids.insert(format!("{key}_WEAK"), weak.kind_identity(false));
        context.weakening.push(weak);
        input
            .install(&mut context.startup, &mut context.profiles, false)
            .unwrap();
        context.native.push(input);
        let result =
            Arc::new(PreparedNativeProfile::check_definition(&definition, output).unwrap());
        result
            .install(&mut context.startup, &mut context.profiles, false)
            .unwrap();
        offers.push(result.offer(false).unwrap());
        ids.insert(format!("{key}_ADMIT"), result.kind_identity(false));
        context.native.push(result);
    }
    (context, ids, offers)
}

pub(super) fn startup_definition() -> String {
    declarations::exact_epoch_declarations()
        + "\n"
        + include_str!("../../../speech/fargan_epoch_feedback.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_feature_epoch_contracts.conduit")
        + "\n"
        + include_str!("../../../speech/fargan_native_startup_contracts.conduit")
}

#[test]
fn startup_domains_retain_separate_exact_native_admissions() {
    let (context, ids, _) = prepare_startup_profiles();
    assert_eq!(ids.len(), 6);
    let source = include_str!("../../../speech/fargan_native_startup_feedback.conduit");
    for (name, id) in ids {
        assert!(source.contains(&id), "exact startup {name}");
    }
    assert!(context
        .native
        .iter()
        .all(
            |profile| maximum_prepared_transport_value_bytes(profile.value_type()).unwrap()
                <= 16384
        ));
    let checked = check_syntax_document(&parse_syntax_document(source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-native-startup-feedback",
        &context.profiles,
    )
    .unwrap();
    assert_eq!(graph.expanded.gears.len(), 9);
}
