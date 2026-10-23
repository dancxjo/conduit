//! Finite-only contracts reach real imported Source/Plan/Play and terminal output gates.
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/scalar_fixture.rs"]
mod fixture;
use conduit_ai::{imported_model_descriptor::ImportedModelInspection, ModelTensorNumericPolicy};
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::*;
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_std_host::ExternalForeInput;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn source(finite: bool) -> String {
    let source = include_str!("fixtures/model-scalar-f32.conduit");
    assert_eq!(source.matches("numeric_policy: ieee(empty)").count(), 2);
    if finite {
        source.replace(
            "numeric_policy: ieee(empty)",
            "numeric_policy: finite_only(empty)",
        )
    } else {
        source.into()
    }
}
fn scalar(bits: u32) -> NamedTensorBundle {
    let bytes = bits.to_le_bytes();
    NamedTensorBundle::from_values(vec![NamedTensor {
        name: "scalar".into(),
        value: TensorValue {
            element: TensorElement::F32,
            dimensions: BoundedSequence::new(),
            axes: BoundedSequence::new(),
            content_digest: tensor_content_digest(&bytes),
            backing: TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap()),
        },
    }])
    .unwrap()
}
#[test]
fn finite_source_imported_plan_and_play_preserve_policy_and_refuse_before_provider_effects() {
    let mut plans = Vec::new();
    for finite in [false, true] {
        for bits in [0x3fa0_0000, 0x7fc0_0001, 0x7f80_0000, 0xff80_0000] {
            let calls = Arc::new(AtomicUsize::new(0));
            let (mut host, plan) = common::host_plan(&source(finite), |signature| {
                Box::new(fixture::prepare(
                    signature,
                    TensorElement::F32,
                    calls.clone(),
                ))
            });
            if bits == 0x3fa0_0000 {
                plans.push(plan.plan_id.clone());
            }
            let placement = &plan.fragments[0].placements[0];
            let inspection = ImportedModelInspection::from_planned_gear(placement).unwrap();
            assert_eq!(inspection.inputs[0].graph_name, "x");
            assert_eq!(inspection.outputs[0].graph_name, "y");
            let signature = conduit_ai::model_infer_catalog::configured_model_signature(
                &placement.configuration,
            )
            .unwrap();
            let conduit_ai::ModelValueConstraint::Tensor(tensor) = &signature.inputs.get()[0].value
            else {
                panic!("tensor")
            };
            assert_eq!(
                tensor.constraint().numeric_policy,
                if finite {
                    ModelTensorNumericPolicy::FiniteOnly
                } else {
                    ModelTensorNumericPolicy::Ieee
                }
            );
            let input = scalar(bits);
            let before = calls.load(Ordering::SeqCst);
            let mut output = common::Outputs::default();
            let result = host.run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input.clone().encode().unwrap(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut common::NoTimer,
            );
            if finite && bits != 0x3fa0_0000 {
                let error = result.expect_err("nonfinite input must refuse");
                assert!(error.contains("code: InvalidInput"), "{error}");
                assert!(output.0.is_empty());
                assert_eq!(calls.load(Ordering::SeqCst), before);
            } else {
                let report = result.unwrap();
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
                assert_eq!(output.0, [input]);
                assert_eq!(calls.load(Ordering::SeqCst), before + 1);
            }
        }
    }
    assert_ne!(
        plans[0], plans[1],
        "selected Plan must bind the numeric contract"
    );
}
#[test]
fn actual_provider_nonfinite_reply_is_terminal_without_publishing_output() {
    for bits in [0x7fc0_0001, 0x7f80_0000, 0xff80_0000] {
        let calls = Arc::new(AtomicUsize::new(0));
        let reply = scalar(bits).encode().unwrap();
        let (mut host, plan) = common::host_plan(&source(true), |signature| {
            Box::new(fixture::prepare_reply(
                signature,
                TensorElement::F32,
                calls.clone(),
                Some(reply.clone()),
            ))
        });
        let mut output = common::Outputs::default();
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: scalar(0x3fa0_0000).encode().unwrap(),
            }],
            &mut output,
            &mut Vec::new(),
            &mut common::NoTimer,
        );
        let error = result.expect_err("nonfinite provider result must refuse");
        assert!(error.contains("BackFailed"), "{error}");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(output.0.is_empty());
    }
}

#[test]
fn source_cannot_omit_the_canonical_numeric_policy_and_get_a_hidden_default() {
    let old = source(false).replace(", numeric_policy: ieee(empty)", "");
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profile = conduit_plot::ProfileCatalog::new();
    conduit_ai::model_infer_catalog::install_model_infer_catalog(&mut startup, &mut profile)
        .unwrap();
    let syntax = conduit_plot::parse_syntax_document(&old);
    assert!(conduit_plot::check_syntax_document(&syntax, &startup).is_err());
}
