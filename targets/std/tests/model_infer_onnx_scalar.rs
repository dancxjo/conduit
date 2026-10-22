//! Exact rank-zero values traverse ordinary checked Source, Plan and Play.
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/scalar_fixture.rs"]
mod fixture;
use conduit_ai::imported_model_descriptor::ImportedModelInspection;
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_std_host::ExternalForeInput;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
fn source(element: TensorElement) -> &'static str {
    match element {
        TensorElement::I64 => include_str!("fixtures/model-scalar-i64.conduit"),
        TensorElement::F32 => include_str!("fixtures/model-scalar-f32.conduit"),
        _ => panic!("two exact fixture profiles"),
    }
}
fn scalar(element: TensorElement) -> TensorValue {
    let bytes = match element {
        TensorElement::I64 => (-17_i64).to_le_bytes().to_vec(),
        TensorElement::F32 => 1.25_f32.to_le_bytes().to_vec(),
        _ => panic!("fixture dtype"),
    };
    TensorValue {
        element,
        dimensions: BoundedSequence::new(),
        axes: BoundedSequence::new(),
        content_digest: tensor_content_digest(&bytes),
        backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
    }
}
fn raw_bundle(values: Vec<NamedTensor>) -> Vec<u8> {
    // This bypasses semantic TensorValue::validate only for negative tests;
    // Native structural encoding still admits a well-formed typed envelope.
    NamedTensorBundle::new(BoundedSequence::try_from_iter(values).unwrap())
        .unwrap()
        .encode()
        .unwrap()
}
#[test]
fn integer_and_floating_scalars_keep_rank_zero_through_imported_source_plan_play() {
    for element in [TensorElement::I64, TensorElement::F32] {
        let calls = Arc::new(AtomicUsize::new(0));
        let (mut host, plan) = common::host_plan(source(element), |signature| {
            Box::new(fixture::prepare(signature, element, calls.clone()))
        });
        let placement = &plan.fragments[0].placements[0];
        let inspection = ImportedModelInspection::from_planned_gear(placement).unwrap();
        assert_eq!(inspection.inputs[0].semantic_port, "scalar");
        assert_eq!(inspection.inputs[0].graph_name, "x");
        assert_eq!(inspection.outputs[0].graph_name, "y");
        let signature =
            conduit_ai::model_infer_catalog::configured_model_signature(&placement.configuration)
                .unwrap();
        let conduit_ai::ModelValueConstraint::Tensor(input) = &signature.inputs.get()[0].value
        else {
            panic!("tensor");
        };
        assert!(input.constraint().axes.get().is_empty());
        assert_eq!(input.constraint().elements.get().as_slice(), [element]);
        assert_eq!(input.constraint().maximum_bytes, element.byte_width());
        let expected = NamedTensorBundle::from_values(vec![NamedTensor {
            name: "scalar".into(),
            value: scalar(element),
        }])
        .unwrap();
        let mut outputs = common::Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: expected.clone().encode().unwrap(),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut common::NoTimer,
            )
            .unwrap();
        assert!(matches!(
            report.observations.last().map(|o| &o.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        let kernel = report.kernel.unwrap();
        assert_eq!(kernel.active_play.plan_id, plan.plan_id);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert_eq!(outputs.0, [expected]);
        assert!(outputs.0[0].get()[0].value.dimensions.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        println!(
            "scalar-import dtype={element:?} source={} plan={} play={} graph={:?} sidecar={:?}",
            plan.source_document_id.as_str(),
            plan.plan_id.as_str(),
            kernel.active_play_id.as_str(),
            inspection.checkpoint.graph,
            inspection.checkpoint.sidecar
        );
    }
}
#[test]
fn wrong_scalar_rank_dtype_element_storage_and_bundle_count_never_invoke_provider() {
    for element in [TensorElement::I64, TensorElement::F32] {
        for case in 0..6 {
            let calls = Arc::new(AtomicUsize::new(0));
            let (mut host, plan) = common::host_plan(source(element), |signature| {
                Box::new(fixture::prepare(signature, element, calls.clone()))
            });
            let mut tensor = scalar(element);
            match case {
                0 => {
                    tensor.dimensions = BoundedSequence::try_from_iter([1]).unwrap();
                    tensor.axes = BoundedSequence::try_from_iter([TensorAxis {
                        role: TensorAxisRole::Feature,
                        identity: None,
                        unit: None,
                    }])
                    .unwrap();
                }
                1 => {
                    tensor.element = if element == TensorElement::I64 {
                        TensorElement::F64
                    } else {
                        TensorElement::I32
                    };
                }
                2 | 3 | 4 => {
                    let width = element.byte_width() as usize;
                    let length = match case {
                        2 => 2 * width,
                        3 => width - 1,
                        _ => width + 1,
                    };
                    let bytes = vec![0; length];
                    tensor.content_digest = tensor_content_digest(&bytes);
                    tensor.backing = TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap());
                }
                _ => {}
            }
            let mut values = vec![NamedTensor {
                name: "scalar".into(),
                value: tensor,
            }];
            if case == 5 {
                values.push(NamedTensor {
                    name: "extra".into(),
                    value: scalar(element),
                });
            }
            let mut outputs = common::Outputs::default();
            let result = host.run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: raw_bundle(values),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut common::NoTimer,
            );
            assert!(result.is_err(), "dtype={element:?} case={case} must refuse");
            assert_eq!(calls.load(Ordering::SeqCst), 0, "case={case}");
            assert!(outputs.0.is_empty(), "case={case}");
        }
    }
}

#[test]
fn invalid_scalar_provider_replies_emit_no_output() {
    for case in 0..4 {
        let element = TensorElement::I64;
        let mut tensor = scalar(element);
        match case {
            0 => {
                tensor.dimensions = BoundedSequence::try_from_iter([1]).unwrap();
                tensor.axes = BoundedSequence::try_from_iter([TensorAxis {
                    role: TensorAxisRole::Feature,
                    identity: None,
                    unit: None,
                }])
                .unwrap();
            }
            1 => {
                tensor.element = TensorElement::F64;
            }
            2 | 3 => {
                let bytes = vec![0; if case == 2 { 16 } else { 7 }];
                tensor.content_digest = tensor_content_digest(&bytes);
                tensor.backing = TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap());
            }
            _ => unreachable!(),
        }
        let reply = raw_bundle(vec![NamedTensor {
            name: "scalar".into(),
            value: tensor,
        }]);
        let calls = Arc::new(AtomicUsize::new(0));
        let (mut host, plan) = common::host_plan(source(element), |signature| {
            Box::new(fixture::prepare_reply(
                signature,
                element,
                calls.clone(),
                Some(reply),
            ))
        });
        let mut outputs = common::Outputs::default();
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: raw_bundle(vec![NamedTensor {
                    name: "scalar".into(),
                    value: scalar(element),
                }]),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut common::NoTimer,
        );
        assert!(result.is_err(), "case={case}");
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "one deliberately faulty provider reply"
        );
        assert!(outputs.0.is_empty(), "case={case}");
    }
}

#[test]
fn scalar_only_offer_admits_zero_rank_and_refuses_rank_one() {
    let mut offer = common::offer();
    offer.limits.maximum_rank = 0;
    let mut requirement = common::requirement();
    requirement.rank = 0;
    offer.validate().unwrap();
    requirement.validate().unwrap();
    assert_eq!(offer.admits(&requirement), Ok(()));
    requirement.rank = 1;
    assert_eq!(
        offer.admits(&requirement),
        Err(conduit_ai::ModelComputeRefusal::UnsupportedShape)
    );
}
