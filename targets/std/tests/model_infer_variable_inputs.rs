//! Reference echo fixture checks admission accounting only, not model numerics.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_core::{ConnectionTrack, ObservationKind, TerminalDisposition};
use conduit_data::{tensor_content_digest, TensorBacking};
use conduit_plot::rust_binding::{BoundedBytes, BoundedSequence, NativeRustBinding};
use conduit_std_host::{
    hosted_model_compute::ReferenceModelComputeAdapter, hosted_model_infer::PreparedModelInfer,
    ExternalForeInput,
};

#[test]
fn changing_actual_input_size_does_not_shrink_the_prepared_admission() {
    let source = include_str!("fixtures/model-linear.conduit")
        .replace("test/linear-model@1", "test/bounded-echo-fixture@1")
        .replace("fixed({ value: 2 })", "bounded({ maximum: 2, minimum: 1 })")
        .replace("identity: \"output\"", "identity: \"input\"");
    let (mut host, plan) = fixture::host_plan(&source, |signature| {
        Box::new(
            PreparedModelInfer::prepare(
                fixture::model(signature),
                ReferenceModelComputeAdapter::new(
                    fixture::offer(),
                    fixture::runtime("fixture/bounded-echo"),
                )
                .unwrap(),
                fixture::requirement(),
                "fixture/bounded-echo@1".into(),
            )
            .unwrap(),
        )
    });
    for width in [1, 2, 1] {
        let mut input = fixture::input([2., 3.]);
        let bytes = [2_f32, 3.]
            .into_iter()
            .take(width)
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>();
        input.dimensions = BoundedSequence::try_from_iter([width as u64]).unwrap();
        input.content_digest = tensor_content_digest(&bytes);
        input.backing = TensorBacking::Inline(BoundedBytes::new(&bytes).unwrap());
        let bundle = fixture::bundle(input);
        let expected = bundle.clone();
        let mut outputs = fixture::Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "inputs".into(),
                    track: ConnectionTrack::Payload,
                    bytes: bundle.encode().unwrap(),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut fixture::NoTimer,
            )
            .unwrap();
        assert!(matches!(
            report.observations.last().map(|event| &event.kind),
            Some(ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            })
        ));
        assert_eq!(outputs.0, [expected]);
    }
}
