//! Source startup policy operates on three separately bounded admitted domains.
use super::*;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn align_native_pcm(epochs: &[StructuredInfoValue]) -> (Vec<i16>, String) {
    let source = declarations::exact_epoch_declarations()
        + "\n"
        + &include_str!("../../../speech/fargan_native_pcm_alignment.conduit")
            .replace("native_epochs: U64\n", "native_epochs: U64 = 63\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-native-pcm-alignment",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(hex) = &graph.expanded.gears[0].configuration[0].value else {
        panic!("alignment Source")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let mut pcm = vec![];
    assert_eq!(epochs.len(), 64);
    for (index, epoch) in epochs.iter().enumerate() {
        assert_eq!(epoch.value_type(), &program.input_type);
        let input = epoch.canonical_bytes().unwrap();
        let output = prepared.evaluate(&input).unwrap();
        assert_eq!(output, program.evaluate(&input).unwrap());
        let output = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        let StructuredInfoValueShape::Collection(samples) =
            super::case_state::field(&output, "samples").shape()
        else {
            panic!("Source aligned PCM")
        };
        assert_eq!(
            samples.len(),
            if index == 0 || index == 63 { 80 } else { 160 }
        );
        for sample in samples {
            let StructuredInfoValueShape::Leaf(bytes) = sample.shape() else {
                panic!("PCM16")
            };
            pcm.push(i16::from_le_bytes(bytes.try_into().unwrap()));
        }
    }
    assert_eq!(pcm.len(), 10080);
    (pcm, source)
}

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
    assert!(context.native.iter().all(|profile| {
        maximum_prepared_transport_value_bytes(profile.value_type()).unwrap() <= 16384
    }));
    let checked = check_syntax_document(&parse_syntax_document(source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-native-startup-feedback",
        &context.profiles,
    )
    .unwrap();
    assert_eq!(graph.expanded.gears.len(), 9);
}

#[test]
fn native_pcm_alignment_is_checked_source_with_exact_variable_block_bounds() {
    let source = declarations::exact_epoch_declarations()
        + "\n"
        + &include_str!("../../../speech/fargan_native_pcm_alignment.conduit")
            .replace("native_epochs: U64\n", "native_epochs: U64 = 63\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let aligned = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "FarganAlignedPcmBlock")
        .unwrap();
    assert!(maximum_prepared_transport_value_bytes(&aligned.value_type).unwrap() < 4096);
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/fargan-native-pcm-alignment",
        &ProfileCatalog::new(),
    )
    .unwrap();
    let ConfigurationValue::Text(hex) = &graph.expanded.gears[0].configuration[0].value else {
        panic!("alignment Source")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let fixture = declarations::fixture_value(&program.input_type);
    let StructuredInfoValueShape::Record(fields) = fixture.shape() else {
        panic!("PCM epoch")
    };
    fn ramp(ty: &StructuredInfoType) -> StructuredInfoValue {
        match ty.shape() {
            StructuredInfoTypeShape::Nominal { representation, .. } => {
                StructuredInfoValue::nominal(ty.clone(), ramp(representation)).unwrap()
            }
            StructuredInfoTypeShape::Collection { element, length } => {
                StructuredInfoValue::collection(
                    ty.clone(),
                    (0..length)
                        .map(|index| {
                            StructuredInfoValue::leaf(
                                element.clone(),
                                i16::try_from(index).unwrap().to_le_bytes().to_vec(),
                            )
                            .unwrap()
                        })
                        .collect(),
                )
                .unwrap()
            }
            _ => panic!("exact PCM ramp"),
        }
    }
    for (epoch, count) in [(1u64, 80usize), (2, 160), (63, 160), (64, 80)] {
        let input = StructuredInfoValue::record(
            program.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = if field.name() == "epoch" {
                        StructuredInfoValue::leaf(
                            field.value().value_type().clone(),
                            epoch.to_le_bytes().to_vec(),
                        )
                        .unwrap()
                    } else if field.name() == "pcm_i16" {
                        ramp(field.value().value_type())
                    } else {
                        field.value().clone()
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let reference = program.evaluate(&input).unwrap();
        assert_eq!(prepared.evaluate(&input).unwrap(), reference);
        let output = StructuredInfoValue::from_canonical_bytes(&reference).unwrap();
        let StructuredInfoValueShape::Collection(values) =
            super::case_state::field(&output, "samples").shape()
        else {
            panic!("aligned PCM")
        };
        assert_eq!(values.len(), count);
        let expected_start = if epoch == 1 { 80 } else { 0 };
        for (value, expected) in values.iter().zip(expected_start..expected_start + count) {
            let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                panic!("I16")
            };
            assert_eq!(
                i16::from_le_bytes(bytes.try_into().unwrap()),
                i16::try_from(expected).unwrap()
            );
        }
    }
}
