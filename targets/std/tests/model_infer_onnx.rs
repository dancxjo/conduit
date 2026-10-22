//! Imported graph/name custody participates in ordinary Source selection/Play.
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/fixture.rs"]
mod fixture;
use conduit_ai::model_infer_catalog::configured_model_signature;
use conduit_core::*;
use conduit_data::{tensor_content_digest, NamedTensor, NamedTensorBundle, TensorBacking};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_std_host::{hosted_model_work::HostedModelWorkAdapter, ExternalForeInput};
use std::{cell::RefCell, collections::BTreeMap};
const SOURCE: &str = include_str!("fixtures/model-multi-linear.conduit");

#[test]
fn checked_source_executes_exact_imported_graph_with_checkpoint_local_names() {
    let selected = RefCell::new(None);
    let (mut host, plan) = common::host_plan(SOURCE, |signature| {
        let imported = fixture::imported(signature, false, false);
        println!("imported-model graph-sha256={:?} signature={:?} sidecar={:?} input-map={:?} output-map={:?}", imported.metadata().sha256, imported.resource().signature().semantic_digest().unwrap(), imported.sidecar_identity(), imported.sidecar().inputs, imported.sidecar().outputs);
        let descriptor = imported.resource().descriptor_identity();
        let sidecar = imported.sidecar_identity();
        let prepared = fixture::prepare(imported);
        assert_eq!(
            prepared.imported_model().unwrap().sidecar_identity(),
            sidecar
        );
        let capability = prepared.capability_offer().clone();
        let resource = prepared.resource_offer().clone();
        selected.replace(Some((descriptor, sidecar, capability, resource)));
        Box::new(prepared)
    });
    let (descriptor, sidecar, capability, resource) = selected.into_inner().unwrap();
    let placement = &plan.fragments[0].placements[0];
    let inspection =
        conduit_ai::imported_model_descriptor::ImportedModelInspection::from_planned_gear(
            placement,
        )
        .unwrap();
    assert_eq!(inspection.graph_name, "tiny-add-sub");
    assert_eq!(
        inspection
            .inputs
            .iter()
            .map(|p| (p.semantic_port.as_str(), p.graph_name.as_str()))
            .collect::<Vec<_>>(),
        [("left", "x"), ("right", "y")]
    );
    assert_eq!(
        inspection
            .outputs
            .iter()
            .map(|p| (p.semantic_port.as_str(), p.graph_name.as_str()))
            .collect::<Vec<_>>(),
        [("upper", "sum"), ("lower", "difference")]
    );
    let mut custody = [0; 96];
    custody[..32].copy_from_slice(&descriptor);
    custody[32..64].copy_from_slice(&sidecar);
    custody[64..].copy_from_slice(&inspection.admission_identity);
    let expected = semantic_digest("model/imported-inference-selection@2", &custody);
    let identity = expected
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    assert_eq!(
        capability.capability_id.as_str(),
        format!("model/infer/{identity}")
    );
    assert_eq!(
        resource.class_id.as_str(),
        format!("model/infer/{identity}")
    );
    assert_eq!(placement.capability_id, capability.capability_id);
    assert_eq!(placement.artifact_id, capability.implementation.artifact_id);
    assert!(placement.resources.iter().any(
        |binding| binding.class_id == resource.class_id && binding.pool_id == resource.pool_id
    ));
    assert_eq!(
        plan.source_document_id,
        conduit_plot::parse_syntax_document(SOURCE).source_document_id()
    );
    assert_eq!(placement.host_id, plan.fragments[0].host_id);
    assert_eq!(placement.boot_id, plan.fragments[0].boot_id);
    let checked_signature = configured_model_signature(&placement.configuration).unwrap();
    assert_eq!(capability.realization_properties.len(), 2);
    assert_eq!(
        checked_signature.inputs.get().as_slice()[0].identity.get(),
        "left"
    );
    let scalar = |value: f32| {
        let mut tensor = common::input([value, 0.]);
        tensor.dimensions = BoundedSequence::try_from_iter([1]).unwrap();
        let bytes = value.to_le_bytes();
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
    let mut output = common::Outputs::default();
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: inputs.encode().unwrap(),
            }],
            &mut output,
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
    assert!(report
        .observations
        .iter()
        .all(|o| o.plan_id.as_ref() == Some(&plan.plan_id)
            && o.active_play_id.as_ref() == Some(&kernel.active_play_id)));
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(output.0.len(), 1);
    let values = output.0[0]
        .get()
        .iter()
        .map(|named| {
            let TensorBacking::Inline(bytes) = &named.value.backing else {
                panic!("inline output")
            };
            (
                named.name.as_str(),
                f32::from_le_bytes(bytes.as_slice().try_into().unwrap()),
            )
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(values, BTreeMap::from([("upper", 5.), ("lower", -1.)]));
    println!(
        "imported-onnx source={} checked={} expanded={} plan={} play={} host={} boot={} capability={} resource={} sidecar={sidecar:?}",
        plan.source_document_id.as_str(),
        plan.checked_plot_id.as_str(),
        plan.expanded_plot_id.as_str(),
        plan.plan_id.as_str(),
        kernel.active_play_id.as_str(),
        plan.fragments[0].host_id.as_str(),
        plan.fragments[0].boot_id.as_str(),
        capability.capability_id.as_str(),
        resource.pool_id.as_str()
    );
}

#[test]
fn changed_graph_or_name_correspondence_changes_the_selected_plan() {
    let prepare = |swapped, reverse| {
        common::host_plan(SOURCE, |signature| {
            Box::new(fixture::prepare(fixture::imported(
                signature, swapped, reverse,
            )))
        })
        .1
    };
    let original = prepare(false, false);
    let different_graph = prepare(true, false);
    let different_sidecar = prepare(false, true);
    for changed in [different_graph, different_sidecar] {
        assert_ne!(original.plan_id, changed.plan_id);
        assert_ne!(
            original.fragments[0].placements[0].capability_id,
            changed.fragments[0].placements[0].capability_id
        );
        assert_eq!(original.source_document_id, changed.source_document_id);
    }
}
