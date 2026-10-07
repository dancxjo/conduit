//! Native realization preparation and ordinary Source runtime proof.
use super::*;

enum NativeRuntimeResource<'a> {
    Model(&'a super::super::custody::RetainedTensor),
    Analysis(Resource),
}
impl RuntimeTensor for NativeRuntimeResource<'_> {
    fn value_type(&self) -> &StructuredInfoType {
        match self {
            Self::Model(value) => value.value_type(),
            Self::Analysis(value) => value.value_type(),
        }
    }
    fn tensor(&self) -> &conduit_data::TensorValue {
        match self {
            Self::Model(value) => value.tensor(),
            Self::Analysis(value) => value.tensor(),
        }
    }
    fn adopt(
        &self,
    ) -> std::sync::Arc<conduit_ai::fixed_tensor_resource::AdmittedFixedTensorResource> {
        match self {
            Self::Model(value) => value.adopt(),
            Self::Analysis(value) => value.adopt(),
        }
    }
}

pub(in super::super) fn run_native_trained_utterance(
    model: &super::super::custody::RetainedSignalModel,
    tape: &super::super::native_session::NativeTape,
    periods: &[StructuredInfoValue],
    proposal: &StructuredInfoValue,
    warm: &[StructuredInfoValue],
) -> Vec<StructuredInfoValue> {
    let seeds = run_native_startup_feedback(warm, proposal);
    let (context, seeded, _) = super::super::prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::super::conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) =
        super::super::feature_cycle::prepare_feedback_with(context, seeded);
    let (context, tail, _, mut offers) = super::super::feature_cycle::prepare_tail(context, &ids);
    offers.extend(seeded.offers().cloned());
    let source = super::super::feature_cycle::native_utterance_source(&ids, &tail);
    let mut startup_material = tape.immutable_material.clone();
    startup_material.extend_from_slice(include_bytes!(
        "../../../../speech/fargan_native_control.conduit"
    ));
    startup_material.extend_from_slice(include_bytes!(
        "../../../../speech/fargan_feature_first_flow.conduit"
    ));
    startup_material.extend_from_slice(include_bytes!(
        "../../../../speech/fargan_native_startup_feedback.conduit"
    ));
    startup_material.extend_from_slice(include_bytes!(
        "../../../../speech/fargan_warm_startup.conduit"
    ));
    startup_material.extend_from_slice(&63u64.to_le_bytes());
    for value in [proposal].into_iter().chain(warm) {
        startup_material.extend_from_slice(&value.canonical_bytes().unwrap());
    }
    let basis = model.basis_material(
        &source,
        &startup_material,
        b"Source finite native63 epochs, two explicit continuation epochs; reference float32",
    );
    use sha2::{Digest, Sha256};
    let anchor = super::super::custody::anchor_literal(model, Sha256::digest(&basis).into());
    let source = source
        .replace(
            "selected: FarganModelFrameAnchor\n",
            &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
        )
        .replace("native_epochs: U64\n", "native_epochs: U64 = 63\n");
    let definition = super::super::native_startup::startup_definition();
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&definition),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    let ty = |name: &str| {
        &checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let seed = |name: &str| {
        seeds
            .iter()
            .find(|seed| seed.value_type() == ty(name))
            .unwrap()
            .canonical_bytes()
            .unwrap()
    };
    let event_ty = ty("FarganFeaturePcmEpoch");
    let StructuredInfoTypeShape::Record { fields, .. } = event_ty.shape() else {
        panic!("event")
    };
    let events = tape
        .epochs
        .iter()
        .zip(periods)
        .enumerate()
        .skip(1)
        .map(|(epoch, (native, period))| {
            let samples_type = fields
                .iter()
                .find(|field| field.name() == "samples")
                .unwrap()
                .value_type();
            fn samples(ty: &StructuredInfoType, values: &[i16]) -> StructuredInfoValue {
                match ty.shape() {
                    StructuredInfoTypeShape::Nominal { representation, .. } => {
                        StructuredInfoValue::nominal(ty.clone(), samples(representation, values))
                            .unwrap()
                    }
                    StructuredInfoTypeShape::Collection { element, length } => {
                        assert_eq!(length as usize, values.len());
                        StructuredInfoValue::collection(
                            ty.clone(),
                            values
                                .iter()
                                .map(|value| {
                                    StructuredInfoValue::leaf(
                                        element.clone(),
                                        value.to_le_bytes().to_vec(),
                                    )
                                    .unwrap()
                                })
                                .collect(),
                        )
                        .unwrap()
                    }
                    _ => panic!("PCM80"),
                }
            }
            let values = BTreeMap::from([
                ("samples", samples(samples_type, &native.samples)),
                ("period", period.clone()),
                (
                    "epoch",
                    StructuredInfoValue::leaf(
                        StructuredInfoType::leaf(kind_id("value/u64")).unwrap(),
                        (epoch as u64).to_le_bytes().to_vec(),
                    )
                    .unwrap(),
                ),
            ]);
            let event = StructuredInfoValue::record(
                event_ty.clone(),
                fields
                    .iter()
                    .map(|field| {
                        StructuredFieldValue::new(field.name(), values[field.name()].clone())
                            .unwrap()
                    })
                    .collect(),
            )
            .unwrap();
            let encoded = event.canonical_bytes().unwrap();
            super::super::interface::admit_retained_session_native(
                &checked,
                "FarganFeaturePcmEpoch",
                &encoded,
            )
            .unwrap();
            encoded
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 62);
    let inputs = BTreeMap::from([
        (
            "signal_seed".into(),
            vec![seed("FarganSignalEpochFeedback")],
        ),
        (
            "conditioning_seed".into(),
            vec![seed("FarganConditioningEpochFeedback")],
        ),
        (
            "feature_seed".into(),
            vec![seed("FarganFeatureEpochFeedback")],
        ),
        ("events".into(), events),
    ]);
    let (plan, context) = super::super::prepare_authored_epoch_entry(
        context,
        source.clone(),
        "speech/flow-fargan-native-utterance",
        true,
        offers,
    )
    .unwrap();
    if let Ok(directory) = std::env::var("CONDUIT_FARGAN_NATIVE_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("session-basis.bin"), &basis).unwrap();
        std::fs::write(directory.join("bound-epoch-source.conduit"), &source).unwrap();
        std::fs::write(directory.join("plan-debug.txt"), format!("{plan:#?}")).unwrap();
    }
    let mut resources = model
        .resources
        .iter()
        .map(|(name, value)| (name.clone(), NativeRuntimeResource::Model(value)))
        .collect::<BTreeMap<_, _>>();
    for (name, resource) in analysis_resources() {
        assert!(resources
            .insert(name, NativeRuntimeResource::Analysis(resource))
            .is_none());
    }
    assert_eq!(resources.len(), 36);
    let result = run_epoch_stream_plan(
        plan,
        &context,
        &resources,
        inputs,
        Some(seeded),
        64,
        ExecutionMode::Normal,
    )
    .unwrap();
    assert!(result.drained);
    assert_eq!(result.values.len(), 64);
    eprintln!(
        "actual committed native utterance trained Source: {}nodes/{}cords; prep{:?}/execute{:?}; exact native63 epochs and two Source continuation epochs",
        result.nodes, result.cords, result.preparation, result.execution
    );
    result.values
}
