//! Genuine optional inputs retain ordinary Source/Plan/Play and exact replies.
#[path = "imported_model_infer/optional_allocation.rs"]
mod allocation;
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/optional_fixture.rs"]
mod fixture;
use conduit_ai::{model_infer_catalog::configured_model_signature, ModelPortPresence};
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::{NamedTensor, NamedTensorBundle};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::ExternalForeInput;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn input(optional: bool) -> NamedTensorBundle {
    let mut values = vec![NamedTensor {
        name: "input".into(),
        value: common::input([2., 3.]),
    }];
    if optional {
        values.push(NamedTensor {
            name: "hint".into(),
            value: common::input([40., 50.]),
        });
    }
    NamedTensorBundle::from_values(values).unwrap()
}

#[test]
fn absent_present_absent_preserves_selected_presence_and_exact_graph_outputs() {
    let source = include_str!("fixtures/model-optional-identity.conduit");
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut host, plan) = common::host_plan(source, |signature| {
        Box::new(fixture::prepare(signature, calls.clone()))
    });
    let placement = &plan.fragments[0].placements[0];
    let signature = configured_model_signature(&placement.configuration).unwrap();
    assert_eq!(
        signature.inputs.get().as_slice()[1].presence,
        ModelPortPresence::Optional
    );
    let refusal = match fixture::prepare_bounded(signature, calls.clone(), 8) {
        Err(refusal) => refusal,
        Ok(_) => panic!("absence cannot reduce the admitted maximum input set"),
    };
    assert!(refusal.contains("model signature exceeds admitted tensor bytes"));
    let inspected =
        conduit_ai::imported_model_descriptor::ImportedModelInspection::from_planned_gear(
            placement,
        )
        .unwrap();
    assert_eq!(inspected.graph_name, "optional-identity");
    assert_eq!(inspected.inputs[1].semantic_port, "hint");
    assert_eq!(inspected.inputs[1].graph_name, "hint");
    for optional in [false, true, false] {
        let mut outputs = common::Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input(optional).encode().unwrap(),
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
        // Identity(x) has one output. The unused optional input must never leak.
        assert_eq!(outputs.0, [input(false)]);
        println!("imported-optional source={} checked={} expanded={} plan={} play={} host={} boot={} optional={optional}", plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(), plan.plan_id.as_str(), kernel.active_play_id.as_str(), placement.host_id.as_str(), placement.boot_id.as_str());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn missing_required_and_malformed_present_optional_refuse_before_provider_effects() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut host, plan) = common::host_plan(
        include_str!("fixtures/model-optional-identity.conduit"),
        |signature| Box::new(fixture::prepare(signature, calls.clone())),
    );
    let only_hint = NamedTensorBundle::from_values(vec![NamedTensor {
        name: "hint".into(),
        value: common::input([2., 3.]),
    }])
    .unwrap();
    let mut wrong_shape = input(true).get().to_vec();
    let hint = &mut wrong_shape[1].value;
    let bytes = 40_f32.to_le_bytes();
    hint.dimensions = conduit_plot::rust_binding::BoundedSequence::try_from_iter([1]).unwrap();
    hint.content_digest = conduit_data::tensor_content_digest(&bytes);
    hint.backing = conduit_data::TensorBacking::Inline(
        conduit_plot::rust_binding::BoundedBytes::new(&bytes).unwrap(),
    );
    let wrong_shape = NamedTensorBundle::from_values(wrong_shape).unwrap();
    let mut wrong_dtype = input(true).get().to_vec();
    let hint = &mut wrong_dtype[1].value;
    let bytes = [40_i64.to_le_bytes(), 50_i64.to_le_bytes()].concat();
    hint.element = conduit_data::TensorElement::I64;
    hint.content_digest = conduit_data::tensor_content_digest(&bytes);
    hint.backing = conduit_data::TensorBacking::Inline(
        conduit_plot::rust_binding::BoundedBytes::new(&bytes).unwrap(),
    );
    let wrong_dtype = NamedTensorBundle::from_values(wrong_dtype).unwrap();
    let mut encoded = input(true).encode().unwrap();
    let wrong_name = NamedTensorBundle::from_values(vec![NamedTensor {
        name: "unexpected".into(),
        value: common::input([2., 3.]),
    }])
    .unwrap();
    let cases = [
        only_hint.encode().unwrap(),
        wrong_name.encode().unwrap(),
        wrong_shape.encode().unwrap(),
        wrong_dtype.encode().unwrap(),
    ];
    for bytes in cases {
        let mut outputs = common::Outputs::default();
        assert!(host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut common::NoTimer
            )
            .is_err());
        assert!(outputs.0.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
    // A corrupt canonical packet exercises the same pre-effect refusal path.
    encoded.pop();
    let mut outputs = common::Outputs::default();
    assert!(host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: encoded
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut common::NoTimer
        )
        .is_err());
    assert!(outputs.0.is_empty());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn optional_absence_and_presence_have_no_allocator_effects_during_play() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut host, plan) = common::host_plan(
        include_str!("fixtures/model-optional-identity.conduit"),
        |signature| Box::new(fixture::prepare(signature, calls.clone())),
    );
    for optional in [false, true, false] {
        let inputs = [ExternalForeInput {
            front_port_id: "inputs".into(),
            track: ConnectionTrack::Payload,
            bytes: input(optional).encode().unwrap(),
        }];
        let mut sink = allocation::Sink::prepare();
        let mut probe = allocation::Probe::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &inputs,
                &mut sink,
                &mut Vec::new(),
                &mut probe,
            )
            .unwrap();
        assert_eq!(probe.phases, 2);
        assert_eq!(
            probe.counts,
            [0, 0, 0],
            "allocation, reallocation, deallocation"
        );
        assert_eq!(sink.calls, 1);
        assert_eq!(
            NamedTensorBundle::decode(&sink.bytes).unwrap(),
            input(false)
        );
        assert_eq!(report.kernel.unwrap().active_play.plan_id, plan.plan_id);
    }
}

#[test]
fn all_optional_empty_present_empty_executes_the_exact_constant_graph() {
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut host, plan) = common::host_plan(
        include_str!("fixtures/model-optional-constant.conduit"),
        |signature| Box::new(fixture::prepare_constant(signature, calls.clone())),
    );
    let placement = &plan.fragments[0].placements[0];
    let signature = configured_model_signature(&placement.configuration).unwrap();
    assert!(signature
        .inputs
        .get()
        .iter()
        .all(|port| port.presence == ModelPortPresence::Optional));
    let empty = NamedTensorBundle::from_values(vec![]).unwrap();
    for value in [empty.clone(), input(true), empty] {
        let mut sink = allocation::Sink::prepare();
        let mut probe = allocation::Probe::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: value.encode().unwrap(),
                }],
                &mut sink,
                &mut Vec::new(),
                &mut probe,
            )
            .unwrap();
        assert_eq!(probe.phases, 2);
        assert_eq!(probe.counts, [0, 0, 0]);
        assert_eq!(sink.calls, 1);
        assert_eq!(
            NamedTensorBundle::decode(&sink.bytes).unwrap(),
            common::bundle(common::input([7., 11.]))
        );
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
        println!("imported-optional-constant source={} checked={} expanded={} plan={} play={} host={} boot={} supplied={} allocator={:?}", plan.source_document_id.as_str(), plan.checked_plot_id.as_str(), plan.expanded_plot_id.as_str(), plan.plan_id.as_str(), kernel.active_play_id.as_str(), placement.host_id.as_str(), placement.boot_id.as_str(), value.get().len(), probe.counts);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}
