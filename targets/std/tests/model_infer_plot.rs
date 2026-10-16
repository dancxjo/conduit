//! Actual named inference and preparation refusal through authored Source.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_ai::model_infer_catalog::*;
use conduit_ai::*;
use conduit_core::*;
use conduit_data::{
    tensor_content_digest, NamedTensor, NamedTensorBundle, TensorAxisRole, TensorBacking,
    TensorElement,
};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_std_host::hosted_model_infer::prepare_linear_model;
use conduit_std_host::ExternalForeInput;
use fixture::*;
use std::collections::BTreeMap;
fn run_source_model(source: &str, input_bundle: NamedTensorBundle) -> Outputs {
    let (mut host, plan) = host_plan(source, |signature| {
        Box::new(
            prepare_linear_model(model(signature), offer(), runtime("linear"), requirement())
                .unwrap(),
        )
    });
    let mut outputs = Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: input_bundle.encode().unwrap(),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert!(matches!(
        report.observations.last().map(|o| &o.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("ordinary kernel");
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(source).source_document_id()
    );
    assert_eq!(kernel.active_play.plan_id, plan.plan_id);
    assert_eq!(kernel.active_play_id, kernel.identity.active_play_id);
    assert!(report.observations.iter().all(|observation| {
        observation.plan_id.as_ref() == Some(&plan.plan_id)
            && observation.active_play_id.as_ref() == Some(&kernel.active_play_id)
    }));
    println!(
        "model-source={} checked={} expanded={} plan={} play={}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(),
        kernel.active_play_id.as_str()
    );
    for kind in [
        conduit_kernel::KernelEventKind::HostCallRequested,
        conduit_kernel::KernelEventKind::HostCallCompleted,
    ] {
        assert_eq!(
            kernel.kernel_sign.iter().filter(|e| e.kind == kind).count(),
            1
        );
    }
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(outputs.0.len(), 1);
    outputs
}

#[test]
fn source_selects_admitted_model_and_kernel_executes_its_coefficients() {
    let outputs = run_source_model(
        include_str!("fixtures/model-linear.conduit"),
        bundle(input([2., 3.])),
    );
    let output = &outputs.0[0].get()[0];
    assert_eq!(output.name, "output");
    let TensorBacking::Inline(bytes) = &output.value.backing else {
        panic!("inline output");
    };
    let values = bytes
        .as_slice()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect::<Vec<_>>();
    assert_eq!(values, [5., 1.]);
}

#[test]
fn preparation_refuses_ineligible_compute_and_output_below_signature_maximum() {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_model_infer_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("fixtures/model-linear.conduit")),
        &startup,
    )
    .map_err(|e| format!("{}: {}", e.code, e.message))
    .unwrap();
    let authored = expand_canonical_plot_for_authoring(&checked, "linear-model", &profile).unwrap();
    let signature = configured_model_signature(&authored.expanded.gears[0].configuration).unwrap();
    let mut scarce = offer();
    scarce.limits.maximum_working_memory_bytes = 32;
    let error = prepare_linear_model(
        model(signature.clone()),
        scarce,
        runtime("linear"),
        requirement(),
    )
    .err()
    .expect("insufficient memory must refuse before advertisement");
    assert!(error.contains("ResourceBoundExceeded"), "{error}");
    let mut wrong_dtype = offer();
    wrong_dtype.supported_elements = vec![TensorElement::F64];
    let error = prepare_linear_model(
        model(signature.clone()),
        wrong_dtype,
        runtime("linear"),
        requirement(),
    )
    .err()
    .expect("ineligible provider must refuse before advertisement");
    assert!(error.contains("UnsupportedElement"), "{error}");
    let mut short = requirement();
    short.output_bytes = 4;
    let error = prepare_linear_model(model(signature), offer(), runtime("linear"), short)
        .err()
        .expect("all declared output bytes must be admitted before advertisement");
    assert!(
        error.contains("signature exceeds admitted tensor bytes"),
        "{error}"
    );
}

#[test]
fn bounded_dynamic_output_must_fit_compute_admission_before_load() {
    let tensor = ModelTensorConstraint::from_parts(
        vec![TensorElement::F32],
        vec![ModelAxisConstraint::new(
            ModelDimensionConstraint::bounded(4, 1).unwrap(),
            TensorAxisRole::Feature,
        )
        .unwrap()],
        16,
    )
    .unwrap();
    let port = |name: &str| {
        ModelPortConstraint::from_parts(
            name.into(),
            "data/tensor@1".into(),
            ModelPortPresence::Required,
            ModelValueConstraint::tensor(tensor.clone()).unwrap(),
        )
        .unwrap()
    };
    let input = ModelPortConstraint::from_parts(
        "input".into(),
        "data/tensor@1".into(),
        ModelPortPresence::Required,
        ModelValueConstraint::tensor(
            ModelTensorConstraint::from_parts(
                vec![TensorElement::F32],
                vec![ModelAxisConstraint::new(
                    ModelDimensionConstraint::fixed(2).unwrap(),
                    TensorAxisRole::Feature,
                )
                .unwrap()],
                8,
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    let signature = ModelSignature::from_parts(
        "test/dynamic-output@1".into(),
        1,
        vec![ModelOperation::Infer],
        vec![input],
        vec![port("output")],
    )
    .unwrap();
    let adapter = conduit_std_host::hosted_model_compute::ReferenceModelComputeAdapter::new(
        offer(),
        runtime("reference"),
    )
    .unwrap();
    let error = conduit_std_host::hosted_model_infer::PreparedModelInfer::prepare(
        model(signature),
        adapter,
        requirement(),
        "adapter/reference/1".into(),
    )
    .err()
    .expect("finite dynamic maxima must be covered before model load");
    assert!(
        error.contains("signature exceeds admitted tensor bytes"),
        "{error}"
    );
}

#[test]
fn source_named_multi_io_is_numerical_and_independent_of_bundle_order() {
    let scalar = |value: f32| {
        let mut tensor = input([value, 0.]);
        let bytes = value.to_le_bytes();
        tensor.dimensions = BoundedSequence::try_from_iter([1]).unwrap();
        tensor.content_digest = tensor_content_digest(&bytes);
        tensor.backing = TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap());
        tensor
    };
    let inputs = NamedTensorBundle::from_values(vec![
        NamedTensor {
            name: "right".into(),
            value: scalar(3.),
        },
        NamedTensor {
            name: "left".into(),
            value: scalar(2.),
        },
    ])
    .unwrap();
    let outputs = run_source_model(include_str!("fixtures/model-multi-linear.conduit"), inputs);
    assert_eq!(outputs.0[0].get().len(), 2);
    let values = outputs.0[0]
        .get()
        .iter()
        .map(|named| {
            assert_eq!(named.value.element, TensorElement::F32);
            assert_eq!(named.value.dimensions.as_slice(), [1]);
            let TensorBacking::Inline(bytes) = &named.value.backing else {
                panic!("inline scalar");
            };
            (
                named.name.as_str(),
                f32::from_le_bytes(bytes.as_slice().try_into().unwrap()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(values, BTreeMap::from([("upper", 5.), ("lower", 1.)]));
}
