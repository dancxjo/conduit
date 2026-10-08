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
    let (context, source, entry, trace_directory) =
        super::native_trace::prepare_source(context, &ids, &tail);
    let analysis = analysis_resources();
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
    if trace_directory.is_some() {
        // The traced session additionally retains the exact analytic parameter
        // resources and grants; these are never discovered during Play.
        for (name, resource) in &analysis {
            let grant = super::super::fixtures::access(resource.tensor());
            let fields: Vec<(&str, Vec<u8>)> = vec![
                ("name", name.as_bytes().to_vec()),
                (
                    "exact_source_type",
                    resource.value_type.canonical_bytes().unwrap(),
                ),
                ("full_analytic_content", resource.bytes.to_vec()),
                ("complete_read_grant", format!("{grant:?}").into_bytes()),
            ];
            for (name, bytes) in fields {
                startup_material.extend_from_slice(&(name.len() as u64).to_le_bytes());
                startup_material.extend_from_slice(name.as_bytes());
                startup_material.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
                startup_material.extend_from_slice(&bytes);
            }
        }
    }
    let basis = model.basis_material(
        &source,
        &startup_material,
        b"Source finite native63 epochs, two explicit continuation epochs; reference float32",
    );
    use sha2::{Digest, Sha256};
    let basis_identity: [u8; 32] = Sha256::digest(&basis).into();
    let anchor = super::super::custody::anchor_literal(model, basis_identity);
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
    let (plan, context) =
        super::super::prepare_authored_epoch_entry(context, source.clone(), entry, true, offers)
            .unwrap();
    if let Ok(directory) = std::env::var("CONDUIT_FARGAN_NATIVE_OUTPUT") {
        let directory = std::path::PathBuf::from(directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("session-basis.bin"), &basis).unwrap();
        std::fs::write(directory.join("bound-epoch-source.conduit"), &source).unwrap();
        std::fs::write(directory.join("plan-debug.txt"), format!("{plan:#?}")).unwrap();
    }
    if let Some(directory) = &trace_directory {
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join("session-basis.bin"), &basis).unwrap();
        std::fs::write(directory.join("bound-epoch-source.conduit"), &source).unwrap();
        std::fs::write(directory.join("plan-debug.txt"), format!("{plan:#?}")).unwrap();
    }
    let mut resources = model
        .resources
        .iter()
        .map(|(name, value)| (name.clone(), NativeRuntimeResource::Model(value)))
        .collect::<BTreeMap<_, _>>();
    for (name, resource) in analysis {
        assert!(resources
            .insert(name, NativeRuntimeResource::Analysis(resource))
            .is_none());
    }
    assert_eq!(resources.len(), 36);
    let traces = trace_directory
        .as_ref()
        .map(|_| super::native_trace::sinks(&context, &source, model, basis_identity));
    let result = run_epoch_stream_plan_with_trace(
        plan,
        &context,
        &resources,
        inputs,
        Some(seeded),
        StreamRun {
            expected: 64,
            mode: ExecutionMode::Normal,
            trace: traces.as_ref(),
        },
    )
    .unwrap();
    if let (Some(directory), Some(traces)) = (&trace_directory, &traces) {
        assert_eq!(result.scheduler_step_allocations, 0);
        assert_eq!(result.prepared_expression_allocations, 0);
        super::native_trace::write(directory, traces, proposal, warm);
        let metrics = serde_json::json!({
            "original_committed_handoff_sha256":"d4be97a0c8350c72df496cae35f817dd617e449157dbe6a25e5dbda79eee2c89",
            "spoken_ordinals":[0],"complete_greeting":false,"native_epochs":63,"model_rows":64,"source_continuation_epochs":2,
            "source_sha256":format!("{:x}", Sha256::digest(source.as_bytes())),
            "private_basis_sha256":format!("{:x}", Sha256::digest(&basis)),
            "nodes":result.nodes,"cords":result.cords,"drained":result.drained,
            "preparation_seconds":result.preparation.as_secs_f64(),"execution_seconds":result.execution.as_secs_f64(),
            "scheduler_step_allocations":result.scheduler_step_allocations,
            "prepared_expression_allocations":result.prepared_expression_allocations,
            "whole_target_noheap":false,"boot_execution":false,"physical_playback":false,"listening_acceptance":false,
        });
        std::fs::write(
            directory.join("execution-metrics.json"),
            serde_json::to_vec_pretty(&metrics).unwrap(),
        )
        .unwrap();
    }
    assert!(result.drained);
    assert_eq!(result.values.len(), 64);
    eprintln!(
        "actual committed native utterance trained Source: {}nodes/{}cords; prep{:?}/execute{:?}; exact native63 epochs and two Source continuation epochs",
        result.nodes, result.cords, result.preparation, result.execution
    );
    result.values
}
