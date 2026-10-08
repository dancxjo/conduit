//! Separate direct16k Source entrance; legacy8k owners remain unchanged.
use super::feature_profiles::FeaturePcmGeometry;
use super::*;
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn prepare_with(
    context: EpochProfiles,
    seeded: conduitos::seeded_state::SeededStateOperationFactory,
) -> (
    EpochProfiles,
    conduitos::seeded_state::SeededStateOperationFactory,
    BTreeMap<String, String>,
) {
    super::feature_profiles::prepare(context, FeaturePcmGeometry::Direct16k, seeded)
}
fn rebind(source: &str, ids: &BTreeMap<String, String>) -> String {
    let (_, legacy) = super::feature_cycle::prepare_first_feature();
    let mut result = source.to_owned();
    let mut pairs = legacy.iter().collect::<Vec<_>>();
    pairs.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
    for (key, old) in pairs {
        if let Some(new) = ids.get(key) {
            result = result.replace(old, new);
        }
    }
    result
}
fn imports_first(source: &str) -> String {
    let imports = source
        .lines()
        .filter(|l| l.starts_with("with "))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    imports
        + "\n"
        + &source
            .lines()
            .filter(|l| !l.starts_with("with "))
            .collect::<Vec<_>>()
            .join("\n")
}
pub(super) fn analysis_source(ids: &BTreeMap<String, String>) -> String {
    imports_first(
        &(super::feature_cycle::feature_source(true)
            + "\n"
            + include_str!("../../../speech/fargan_direct16k_normalization.conduit")
            + "\n"
            + &rebind(
                include_str!("../../../speech/fargan_feature_direct16k_flow.conduit"),
                ids,
            )),
    )
}
pub(super) fn first_source(ids: &BTreeMap<String, String>) -> String {
    imports_first(
        &(analysis_source(ids)
            + "\n"
            + &rebind(
                include_str!("../../../speech/fargan_feature_first_flow.conduit"),
                ids,
            )
            .replace(
                "speech/flow-fargan-feature-native-analysis",
                "speech/flow-fargan-feature-direct16k-analysis",
            )
            .replace(
                "speech/flow-fargan-feature-first-",
                "speech/flow-fargan-feature-first16k-",
            )),
    )
}
pub(super) fn cycle_source(ids: &BTreeMap<String, String>) -> String {
    imports_first(
        &[
            analysis_source(ids),
            super::feature_cycle::feedback_entry_source(ids),
            include_str!("../../../speech/fargan_epoch_anchor.conduit").into(),
            rebind(
                include_str!("../../../speech/fargan_feature_cycle.conduit"),
                ids,
            )
            .replace(
                "speech/flow-fargan-feature-native-analysis",
                "speech/flow-fargan-feature-direct16k-analysis",
            ),
        ]
        .join("\n"),
    )
}
#[test]
fn direct16k_preparation_preserves_legacy_states_and_distinct_pcm_geometry() {
    let (legacy, _, old) = super::feature_cycle::prepare_feedback();
    let (new, _, ids) = prepare_with(
        prepared_epoch_profiles_with_capacity(true),
        conduitos::seeded_state::SeededStateOperationFactory::default(),
    );
    for key in ["STATE", "PROPOSAL", "PENDING", "MODEL_EVENT_NATIVE"] {
        let key = if key.ends_with("NATIVE") {
            format!("__FEATURE_{key}__")
        } else {
            format!("__FEATURE_{key}_NATIVE__")
        };
        assert_eq!(ids[&key], old[&key]);
    }
    for key in ["INPUT", "EVENT"] {
        let key = format!("__FEATURE_{key}_NATIVE__");
        assert_ne!(ids[&key], old[&key]);
    }
    for key in ["INPUT", "EVENT"] {
        let identity = &ids[&format!("__FEATURE_{key}_NATIVE__")];
        let profile = new
            .native
            .iter()
            .find(|p| &p.kind_identity(true) == identity)
            .unwrap();
        let encoded = profile.value_type().canonical_bytes().unwrap();
        assert_eq!(
            &StructuredInfoType::from_canonical_bytes(&encoded).unwrap(),
            profile.value_type()
        );
        let maximum = maximum_prepared_transport_value_bytes(profile.value_type()).unwrap();
        assert!(maximum <= 16384);
        eprintln!(
            "direct16k {key} Type{}B/max transport{}B",
            encoded.len(),
            maximum
        );
    }
    let _ = legacy;
    let source = analysis_source(&ids);
    if let Ok(path) = std::env::var("CONDUIT_FARGAN16_SOURCE_DIAGNOSTIC") {
        std::fs::write(path, &source).unwrap();
    }
    let checked = check_syntax_document(&parse_syntax_document(&source), &new.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-feature-direct16k-analysis",
        &new.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() > 80);
    assert!(
        !graph
            .expanded
            .gears
            .iter()
            .any(|g| g.kind_id.as_str().contains("i16-to-f32-80"))
    );
    eprintln!(
        "direct16k analysis:{}nodes/{}cords",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
}

pub(super) fn prepare_first() -> (EpochProfiles, BTreeMap<String, String>) {
    let (mut context, _, mut ids) = prepare_with(
        prepared_epoch_profiles_with_capacity(true),
        conduitos::seeded_state::SeededStateOperationFactory::default(),
    );
    let event = context
        .native
        .iter()
        .find(|p| p.kind_identity(true) == ids["__FEATURE_EVENT_NATIVE__"])
        .unwrap()
        .value_type()
        .clone();
    let weak = std::sync::Arc::new(PreparedNominalWeakening::prepare(event).unwrap());
    weak.install(&mut context.startup, &mut context.profiles, true)
        .unwrap();
    ids.insert(
        "__FEATURE_FIRST_EVENT_WEAK__".into(),
        weak.kind_identity(true),
    );
    context.weakening.push(weak);
    (context, ids)
}

#[test]
fn direct16k_first_epoch_executes_original_feature_and_period_owners() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let periods = super::runtime::run_native_period_controls(&[(false, 20480)]);
            let mut samples = [0i16; 160];
            for (i, sample) in samples.iter_mut().enumerate() {
                *sample = ((i as i32 * 397) % 65536 - 32768) as i16;
            }
            let proposal = super::runtime::run_native_first_feature16k(&samples, &periods[0]);
            assert_eq!(super::case_state::field(&proposal, "period"), &periods[0]);
            let history = super::case_state::field(&proposal, "history");
            let StructuredInfoValueShape::Collection(values) = history.shape() else {
                panic!("history")
            };
            assert_eq!(values.len(), 640);
            assert!(values[..480].iter().all(|v| {
                let StructuredInfoValueShape::Leaf(b) = v.shape() else {
                    panic!("scalar")
                };
                f32::from_le_bytes(b.try_into().unwrap()) == 0.
            }));
            let mut previous = 0.0f32;
            for (sample, value) in samples.iter().zip(&values[480..]) {
                let normalized = f32::from(*sample) / 32768.0;
                let expected = normalized - f32::from_bits(0x3f59999a) * previous;
                let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
                    panic!("scalar")
                };
                assert_eq!(
                    f32::from_le_bytes(bytes.try_into().unwrap()).to_bits(),
                    expected.to_bits()
                );
                previous = normalized;
            }
            let StructuredInfoValueShape::Collection(features) =
                super::case_state::field(&proposal, "features").shape()
            else {
                panic!("feature20")
            };
            assert_eq!(features.len(), 20);
        })
        .unwrap()
        .join()
        .unwrap();
}

pub(super) fn utterance_source(ids: &BTreeMap<String, String>, tail: &str) -> String {
    let native = [
        super::conditioning_cycle::compound_source(),
        cycle_source(ids)
            .replace("type FarganPeriod = U16 in 32..=255\n", "")
            .replace(
                include_str!("../../../speech/fargan_epoch_anchor.conduit"),
                "",
            )
            .lines()
            .filter(|line| {
                !(line.starts_with("with ") && line.ends_with(" as FarganModelFrameAnchor"))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        include_str!("../../../speech/fargan_native_cycle.conduit").into(),
    ]
    .join("\n");
    imports_first(
        &(native
            + "\n"
            + &tail
                .lines()
                .filter(|l| !l.ends_with(" as FarganPcm16EpochResult"))
                .collect::<Vec<_>>()
                .join("\n")
            + "\n"
            + include_str!("../../../speech/fargan_native_utterance.conduit")),
    )
}

/// Preparation-only exact PCM geometry. This retains caller custody material;
/// it does not itself admit a common-IPA carrier or grant measurement authority.
pub(super) struct RetainedDirect16kTape {
    epochs: Vec<[i16; 160]>,
    immutable_material: Vec<u8>,
    service: super::service_profile::PreparedServiceProfile,
}
pub(super) fn explicit_analysis_policy() -> Vec<u8> {
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../speech/fargan_direct16k_policy.conduit"
        )),
        &StartupCatalog::new(),
    )
    .unwrap();
    super::declarations::fixture_value(&checked.native_types[0].value_type)
        .canonical_bytes()
        .unwrap()
}
impl RetainedDirect16kTape {
    pub(super) fn epochs(&self) -> &[[i16; 160]] {
        &self.epochs
    }
    pub(super) fn immutable_material(&self) -> &[u8] {
        &self.immutable_material
    }
    pub(super) fn service(&self) -> &super::service_profile::PreparedServiceProfile {
        &self.service
    }
    pub fn prepare(
        pcm: &[i16],
        original_custody: &[u8],
        selected_policy: Option<&[u8]>,
    ) -> Result<Self, &'static str> {
        Self::prepare_with_service(
            pcm,
            original_custody,
            selected_policy,
            super::service_profile::PreparedServiceProfile::production(),
        )
    }
    pub(super) fn prepare_with_service(
        pcm: &[i16],
        original_custody: &[u8],
        selected_policy: Option<&[u8]>,
        service: super::service_profile::PreparedServiceProfile,
    ) -> Result<Self, &'static str> {
        let policy = selected_policy.ok_or("explicit analysis-resynthesis loss policy required")?;
        let checked = check_syntax_document(
            &parse_syntax_document(include_str!(
                "../../../speech/fargan_direct16k_policy.conduit"
            )),
            &StartupCatalog::new(),
        )
        .map_err(|_| "policy Source")?;
        super::interface::admit_retained_session_native(
            &checked,
            "FarganDirect16kAnalysisPolicy",
            policy,
        )
        .map_err(|_| "foreign or corrupt analysis policy")?;
        if original_custody.is_empty() {
            return Err("original carrier custody required");
        }
        if !pcm.len().is_multiple_of(160) {
            return Err("exact160 frame boundary required; no padding");
        }
        let count = pcm.len() / 160;
        if !(2..=65535).contains(&count) {
            return Err("finite2..65535 epoch profile");
        }
        let mut immutable_material =
            b"fargan/direct16016k/v1; analysis-resynthesis accepted; no clock mapping".to_vec();
        immutable_material.extend_from_slice(&(service.material().len() as u64).to_le_bytes());
        immutable_material.extend_from_slice(service.material());
        immutable_material.extend_from_slice(&(policy.len() as u64).to_le_bytes());
        immutable_material.extend_from_slice(policy);
        immutable_material.extend_from_slice(include_bytes!(
            "../../../speech/fargan_direct16k_policy.conduit"
        ));
        immutable_material.extend_from_slice(&(original_custody.len() as u64).to_le_bytes());
        immutable_material.extend_from_slice(original_custody);
        immutable_material.extend_from_slice(&(pcm.len() as u64).to_le_bytes());
        for sample in pcm {
            immutable_material.extend_from_slice(&sample.to_le_bytes());
        }
        Ok(Self {
            epochs: pcm.as_chunks::<160>().0.to_vec(),
            immutable_material,
            service,
        })
    }
}
#[test]
fn direct16k_tape_retains_original_and_refuses_padding_or_implicit_loss() {
    let pcm = vec![123; 320];
    assert!(RetainedDirect16kTape::prepare(&pcm, b"original", Some(&[0; 4])).is_err());
    assert!(RetainedDirect16kTape::prepare(&pcm, b"original", None).is_err());
    assert!(RetainedDirect16kTape::prepare(&pcm, b"", Some(&explicit_analysis_policy())).is_err());
    assert!(
        RetainedDirect16kTape::prepare(&pcm[..319], b"original", Some(&explicit_analysis_policy()))
            .is_err()
    );
    assert!(
        RetainedDirect16kTape::prepare(&pcm[..160], b"original", Some(&explicit_analysis_policy()))
            .is_err()
    );
    let tape = RetainedDirect16kTape::prepare(&pcm, b"original", Some(&explicit_analysis_policy()))
        .unwrap();
    assert_eq!(tape.epochs.len(), 2);
    assert!(tape.immutable_material.windows(8).any(|s| s == b"original"));
    assert!(tape.epochs.iter().flatten().eq(pcm.iter()));
}

#[test]
fn direct16k_two_hundred_epochs_expand_with_three_original_feedback_owners() {
    let (context, seeded, _) = prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) = prepare_with(context, seeded);
    let (context, tail, _, _) = super::feature_cycle::prepare_tail_for_epochs(context, &ids, 200);
    assert_eq!(seeded.offers().count(), 3);
    let receipt = format!("[{}]", vec!["1"; 32].join(","));
    let anchor = format!(
        "{{artifact_identity:{receipt},model_descriptor_identity:{receipt},session_basis_identity:{receipt},precision:reference_float32(\"\")}}"
    );
    let source = utterance_source(&ids, &tail)
        .replace(
            "selected: FarganModelFrameAnchor\n",
            &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
        )
        .replace("native_epochs: U64\n", "native_epochs: U64 = 200\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &context.startup).unwrap();
    let graph = expand_canonical_plot_for_authoring(
        &checked,
        "speech/flow-fargan-native-utterance",
        &context.profiles,
    )
    .unwrap();
    assert!(graph.expanded.gears.len() > 900);
    assert!(source.contains("native_epochs: U64 = 200"));
    assert!(source.contains("when((.epoch + 1) == native_epochs)"));
    assert!(!source.contains("native_epochs: U64 = 63"));
    eprintln!(
        "direct16016k finite200 Source:{}nodes/{}cords",
        graph.expanded.gears.len(),
        graph.expanded.connections.len()
    );
}

#[test]
#[ignore = "actual retained trained model, typed two-epoch numeric preflight only"]
fn direct16k_two_epoch_trained_numeric_preflight() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let tape = RetainedDirect16kTape::prepare_with_service(
                &[0; 320],
                b"typed numeric preflight; no common-IPA or acoustic evidence claim",
                Some(&explicit_analysis_policy()),
                super::service_profile::PreparedServiceProfile::boundary_test(),
            )
            .unwrap();
            let periods = super::runtime::run_native_period_controls(&[(false, 20480); 2]);
            let proposal =
                super::runtime::run_native_first_feature16k(&tape.epochs()[0], &periods[0]);
            let root =
                std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
            let mut model = super::custody::RetainedSignalModel::load(&root);
            let (_, conditioning) = model.conditioning_resources();
            for (name, resource) in conditioning {
                assert!(
                    model
                        .resources
                        .insert(format!("conditioning_{name}"), resource)
                        .is_none()
                );
            }
            let warm = super::runtime::run_native_warm_startup(&model, &proposal);
            let epochs = super::runtime::run_direct16k_trained_utterance(
                &model, &tape, &periods, &proposal, &warm,
            );
            assert_eq!(epochs.len(), 3);
            let (pcm, _) = super::native_startup::align_native_pcm_for_epochs(&epochs, 2);
            assert_eq!(pcm.len(), 320);
        })
        .unwrap()
        .join()
        .unwrap();
}
