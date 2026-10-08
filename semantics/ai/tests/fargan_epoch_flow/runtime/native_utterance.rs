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
    let samples = tape
        .epochs
        .iter()
        .map(|epoch| epoch.samples.as_slice())
        .collect::<Vec<_>>();
    run_trained_profile(
        model,
        &samples,
        &tape.immutable_material,
        periods,
        proposal,
        warm,
        None,
    )
}

pub(in super::super) fn run_direct16k_trained_utterance(
    model: &super::super::custody::RetainedSignalModel,
    tape: &super::super::direct16k::RetainedDirect16kTape,
    periods: &[StructuredInfoValue],
    proposal: &StructuredInfoValue,
    warm: &[StructuredInfoValue],
) -> Vec<StructuredInfoValue> {
    let samples = tape
        .epochs()
        .iter()
        .map(|epoch| epoch.as_slice())
        .collect::<Vec<_>>();
    run_trained_profile(
        model,
        &samples,
        tape.immutable_material(),
        periods,
        proposal,
        warm,
        Some(tape.service()),
    )
}

fn run_trained_profile(
    model: &super::super::custody::RetainedSignalModel,
    samples: &[&[i16]],
    immutable_material: &[u8],
    periods: &[StructuredInfoValue],
    proposal: &StructuredInfoValue,
    warm: &[StructuredInfoValue],
    service_profile: Option<&super::super::service_profile::PreparedServiceProfile>,
) -> Vec<StructuredInfoValue> {
    let direct16k = service_profile.is_some();
    assert_eq!(samples.len(), periods.len());
    let native_epochs = u32::try_from(samples.len()).unwrap();
    assert!((2..=65535).contains(&native_epochs));
    if !direct16k {
        assert_eq!(native_epochs, 63);
    }
    assert!(samples
        .iter()
        .all(|s| s.len() == if direct16k { 160 } else { 80 }));
    let seeds = run_native_startup_feedback(warm, proposal);
    let (context, seeded, _) = super::super::prepared_signal_cycle_profiles_with_capacity(true);
    let (context, seeded, _) = super::super::conditioning_cycle::prepare_with(context, seeded);
    let (context, seeded, ids) = if direct16k {
        super::super::direct16k::prepare_with(context, seeded)
    } else {
        super::super::feature_cycle::prepare_feedback_with(context, seeded)
    };
    let (context, tail, _, mut offers) =
        super::super::feature_cycle::prepare_tail_for_epochs(context, &ids, native_epochs);
    offers.extend(seeded.offers().cloned());
    let (context, source, entry, trace_directory) = if direct16k {
        (
            context,
            super::super::direct16k::utterance_source(&ids, &tail),
            "speech/flow-fargan-native-utterance",
            None,
        )
    } else {
        super::native_trace::prepare_source(context, &ids, &tail)
    };
    let analysis = analysis_resources();
    let mut startup_material = immutable_material.to_vec();
    if direct16k {
        startup_material.extend_from_slice(include_bytes!(
            "../../../../speech/fargan_direct16k_normalization.conduit"
        ));
        startup_material.extend_from_slice(include_bytes!(
            "../../../../speech/fargan_feature_direct16k_flow.conduit"
        ));
        startup_material.extend_from_slice(include_bytes!(
            "../../../../speech/fargan_feature_direct16k_contracts.conduit"
        ));
    }
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
    startup_material.extend_from_slice(&u64::from(native_epochs).to_le_bytes());
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
        if direct16k { b"Source direct16016k finite epochs, two explicit continuation epochs; reference float32; declared analysis-resynthesis loss" } else { b"Source finite native63 epochs, two explicit continuation epochs; reference float32" },
    );
    use sha2::{Digest, Sha256};
    let basis_identity: [u8; 32] = Sha256::digest(&basis).into();
    let anchor = super::super::custody::anchor_literal(model, basis_identity);
    let source = source
        .replace(
            "selected: FarganModelFrameAnchor\n",
            &format!("selected: FarganModelFrameAnchor = {anchor}\n"),
        )
        .replace(
            "native_epochs: U64\n",
            &format!("native_epochs: U64 = {native_epochs}\n"),
        );
    let mut definition = super::super::native_startup::startup_definition();
    if direct16k {
        definition += "\n";
        definition += include_str!("../../../../speech/fargan_feature_direct16k_contracts.conduit");
    }
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
    let event_ty = ty(if direct16k {
        "FarganFeaturePcmEpoch16k"
    } else {
        "FarganFeaturePcmEpoch"
    });
    let StructuredInfoTypeShape::Record { fields, .. } = event_ty.shape() else {
        panic!("event")
    };
    let events = samples
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
                ("samples", samples(samples_type, native)),
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
                if direct16k {
                    "FarganFeaturePcmEpoch16k"
                } else {
                    "FarganFeaturePcmEpoch"
                },
                &encoded,
            )
            .unwrap();
            encoded
        })
        .collect::<Vec<_>>();
    assert_eq!(events.len(), native_epochs as usize - 1);
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
        super::super::plan_artifact::export(&directory, &plan, &context);
    }
    if let Some(directory) = &trace_directory {
        std::fs::create_dir_all(directory).unwrap();
        std::fs::write(directory.join("session-basis.bin"), &basis).unwrap();
        std::fs::write(directory.join("bound-epoch-source.conduit"), &source).unwrap();
        std::fs::write(directory.join("plan-debug.txt"), format!("{plan:#?}")).unwrap();
        super::super::plan_artifact::export(directory, &plan, &context);
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
    let compare_boundaries = service_profile.is_some_and(|p| p.compares_boundaries());
    let comparison_plan = compare_boundaries.then(|| plan.clone());
    let comparison_inputs = compare_boundaries.then(|| inputs.clone());
    let service = service_profile.map(ServiceBudget::direct).unwrap_or_else(ServiceBudget::legacy);
    let result = run_epoch_stream_plan_with_trace(
        plan,
        &context,
        &resources,
        inputs,
        Some(&seeded),
        StreamRun {
            expected: native_epochs as usize + 1,
            mode: ExecutionMode::Normal,
            trace: traces.as_ref(),
            service,
        },
    )
    .unwrap();
    if compare_boundaries {
        assert!(result.quantum_boundaries >= 2);
        let uninterrupted = run_epoch_stream_plan_with_trace(comparison_plan.unwrap(), &context, &resources, comparison_inputs.unwrap(), Some(&seeded), StreamRun { expected: native_epochs as usize + 1, mode: ExecutionMode::Normal, trace: None, service: ServiceBudget { observe_boundaries: false, ..service } }).unwrap();
        assert_eq!(result.values, uninterrupted.values, "every original canonical Native row survives finite service boundaries exactly");
        assert_eq!(result.service_steps, uninterrupted.service_steps);
        assert!(uninterrupted.drained);
        eprintln!("actual trained finite-boundary equivalence PASS: {} complete Native rows, {} services, {} crossed boundaries; exact same sealed Plan/basis/resources/inputs", result.values.len(), result.service_steps, result.quantum_boundaries);
    }
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
    assert_eq!(result.values.len(), native_epochs as usize + 1);
    eprintln!(
        "actual committed native utterance trained Source: {}nodes/{}cords; prep{:?}/execute{:?}; exact native{} epochs and two Source continuation epochs",
        result.nodes, result.cords, result.preparation, result.execution, native_epochs
    );
    result.values
}
