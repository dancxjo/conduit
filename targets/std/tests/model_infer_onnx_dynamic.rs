//! Dynamic-but-bounded imported outputs through ordinary checked Source/Play.
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/dynamic_fixture.rs"]
mod fixture;
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::{tensor_content_digest, NamedTensorBundle, TensorBacking};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_std_host::ExternalForeInput;

fn input(width: usize) -> NamedTensorBundle {
    let mut input = common::input([2., 3.]);
    let bytes = [2_f32, 3., 4.]
        .into_iter()
        .take(width)
        .flat_map(f32::to_le_bytes)
        .collect::<Vec<_>>();
    input.dimensions = BoundedSequence::try_from_iter([width as u64]).unwrap();
    input.content_digest = tensor_content_digest(&bytes);
    input.backing = TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap());
    common::bundle(input)
}

#[test]
fn imported_identity_keeps_dynamic_outputs_inside_the_admitted_profile() {
    let source = include_str!("fixtures/model-linear.conduit")
        .replace("test/linear-model@1", "test/imported-bounded-identity@1")
        .replace("fixed({ value: 2 })", "bounded({ maximum: 2, minimum: 1 })")
        .replace("identity: \"output\"", "identity: \"input\"");
    let (mut host, plan) =
        common::host_plan(&source, |signature| Box::new(fixture::prepare(signature)));
    let placement = &plan.fragments[0].placements[0];
    assert_eq!(
        plan.source_document_id,
        conduit_plot::parse_syntax_document(&source).source_document_id()
    );
    for width in [1, 2, 1] {
        let expected = input(width);
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
        assert_eq!(
            outputs.0[0].get()[0].value.dimensions.as_slice(),
            [width as u64]
        );
        println!("imported-dynamic source={} checked={} expanded={} plan={} play={} host={} boot={} capability={} width={width}",plan.source_document_id.as_str(),plan.checked_plot_id.as_str(),plan.expanded_plot_id.as_str(),plan.plan_id.as_str(),kernel.active_play_id.as_str(),placement.host_id.as_str(),placement.boot_id.as_str(),placement.capability_id.as_str());
    }
    let mut outputs = common::Outputs::default();
    let error = match host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "inputs".into(),
            track: ConnectionTrack::Payload,
            bytes: input(3).encode().unwrap(),
        }],
        &mut outputs,
        &mut Vec::new(),
        &mut common::NoTimer,
    ) {
        Err(error) => error,
        Ok(_) => panic!("width three exceeds the admitted symbol maximum"),
    };
    assert!(
        error.starts_with("installed kernel step: BackFailed"),
        "{error}"
    );
    assert!(error.contains("code: InvalidInput"), "{error}");
    assert!(error.contains("detail: 72"), "{error}");
    assert!(outputs.0.is_empty());
}
