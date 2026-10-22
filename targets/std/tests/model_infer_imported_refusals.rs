//! Compatibility refusals precede provider load at the production entrance.
#[path = "common_model_infer/mod.rs"]
mod common;
#[path = "imported_model_infer/fixture.rs"]
mod fixture;
use fixture::profile;
use conduit_ai::{
    imported_back_admission::ImportedBackRefusal, imported_onnx::ONNX_FORMAT_PROFILE, *,
};
use conduit_std_host::{
    hosted_model_compute::*,
    hosted_model_infer::{ImportedModelPrepareRefusal, PreparedModelInfer},
};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

struct LoadProbe {
    inner: LinearF32ModelAdapter,
    loads: Arc<AtomicUsize>,
    foreign_reply_runtime: Option<ModelComputeRuntimeIdentity>,
    calls: Arc<AtomicUsize>,
}
impl ModelComputeAdapter for LoadProbe {
    fn offer(&self) -> &ModelComputeOffer {
        self.inner.offer()
    }
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        self.inner.runtime_identity()
    }
    fn load(&mut self, id: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        self.loads.fetch_add(1, Ordering::SeqCst);
        self.inner.load(id, bytes)
    }
    fn output_template(
        &self,
    ) -> Result<Option<conduit_data::NamedTensorBundle>, ModelComputeRefusal> {
        self.inner.output_template()
    }
    fn execute(&mut self, input: ModelComputeInvocation<'_>) -> ModelComputeAdapterTerminal<'_> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut terminal = self.inner.execute(input);
        if let (ModelComputeAdapterTerminal::Produced(reply), Some(runtime)) =
            (&mut terminal, &self.foreign_reply_runtime)
        {
            reply.runtime = runtime;
        }
        terminal
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.inner.cancel()
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.inner.unload()
    }
}

#[test]
fn unsupported_profiles_and_foreign_custody_never_load_a_provider() {
    let _ = common::host_plan(
        include_str!("fixtures/model-multi-linear.conduit"),
        |signature| {
            for case in 0..5 {
                let model = fixture::imported(signature.clone(), false, false);
                let mut offer = common::offer();
                offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
                let loads = Arc::new(AtomicUsize::new(0));
                let adapter = LoadProbe {
                    inner: LinearF32ModelAdapter::new(
                        offer,
                        common::runtime("exact-add-sub-fixture"),
                        [[1., 1.], [1., -1.]],
                        [0., 0.],
                    )
                    .unwrap()
                    .with_signature(&signature)
                    .unwrap(),
                    loads: loads.clone(),
                    foreign_reply_runtime: None,
                    calls: Arc::new(AtomicUsize::new(0)),
                };
                let artifact = "adapter/exact-add-sub-fixture/1";
                let mut profile = profile::exact(
                    &model,
                    adapter.runtime_identity(),
                    artifact,
                    &["Add", "Sub"],
                );
                let mut requirement = common::requirement();
                requirement.model_format = ONNX_FORMAT_PROFILE.into();
                requirement.model_bytes = model.resource().bytes().len() as u64;
                match case {
                    0 => {
                        profile.opsets[0].minimum = 19;
                        profile.opsets[0].maximum = 19;
                    }
                    1 => {
                        profile.operators.retain(|op| op.name != "Sub");
                    }
                    2 => {
                        profile.runtime.runtime_version = "foreign-runtime".into();
                    }
                    3 => {
                        profile.checkpoint.sidecar[0] ^= 1;
                    }
                    _ => {
                        requirement.working_memory_bytes = 2048;
                    }
                }
                let error = match PreparedModelInfer::prepare_imported(
                    model,
                    adapter,
                    requirement,
                    artifact.into(),
                    &profile,
                ) {
                    Err(error) => error,
                    Ok(_) => panic!("invalid imported profile must refuse before load"),
                };
                assert!(matches!(
                    (case, error),
                    (
                        0,
                        ImportedModelPrepareRefusal::Back(
                            ImportedBackRefusal::UnsupportedOpset { .. }
                        )
                    ) | (
                        1,
                        ImportedModelPrepareRefusal::Back(
                            ImportedBackRefusal::UnsupportedOperator(_)
                        )
                    ) | (
                        2,
                        ImportedModelPrepareRefusal::Back(ImportedBackRefusal::RuntimeIdentity)
                    ) | (
                        3,
                        ImportedModelPrepareRefusal::Back(ImportedBackRefusal::Sidecar)
                    ) | (
                        4,
                        ImportedModelPrepareRefusal::Compute(
                            ModelComputeRefusal::ResourceBoundExceeded
                        )
                    )
                ));
                assert_eq!(loads.load(Ordering::SeqCst), 0);
            }
            Box::new(fixture::prepare(fixture::imported(signature, false, false)))
        },
    );
}

#[test]
fn imported_reply_from_unadmitted_runtime_emits_no_output() {
    use conduit_core::ConnectionTrack;
    use conduit_data::{NamedTensor, NamedTensorBundle};
    use conduit_plot::rust_binding::NativeRustBinding;
    use conduit_std_host::ExternalForeInput;
    let loads = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let (mut host, plan) = common::host_plan(
        include_str!("fixtures/model-multi-linear.conduit"),
        |signature| {
            let model = fixture::imported(signature.clone(), false, false);
            let mut offer = common::offer();
            offer.accepted_formats = vec![ONNX_FORMAT_PROFILE.into()];
            let adapter = LoadProbe {
                inner: LinearF32ModelAdapter::new(
                    offer,
                    common::runtime("exact-add-sub-fixture"),
                    [[1., 1.], [1., -1.]],
                    [0., 0.],
                )
                .unwrap()
                .with_signature(&signature)
                .unwrap(),
                loads: loads.clone(),
                foreign_reply_runtime: Some(common::runtime("unadmitted-reply-runtime")),
                calls: calls.clone(),
            };
            let artifact = "adapter/exact-add-sub-fixture/1";
            let profile = profile::exact(
                &model,
                adapter.runtime_identity(),
                artifact,
                &["Add", "Sub"],
            );
            let mut requirement = common::requirement();
            requirement.model_format = ONNX_FORMAT_PROFILE.into();
            requirement.model_bytes = model.resource().bytes().len() as u64;
            Box::new(
                PreparedModelInfer::prepare_imported(
                    model,
                    adapter,
                    requirement,
                    artifact.into(),
                    &profile,
                )
                .unwrap(),
            )
        },
    );
    let mut tensor = common::input([2., 3.]);
    // Split the ordinary two-element fixture into two declared one-element ports.
    tensor.dimensions[0] = 1;
    tensor.backing = conduit_data::TensorBacking::Inline(
        conduit_plot::rust_binding::BoundedBytes::new(&2_f32.to_le_bytes()).unwrap(),
    );
    tensor.content_digest = conduit_data::tensor_content_digest(&2_f32.to_le_bytes());
    let input = NamedTensorBundle::from_values(vec![
        NamedTensor {
            name: "left".into(),
            value: tensor.clone(),
        },
        NamedTensor {
            name: "right".into(),
            value: tensor,
        },
    ])
    .unwrap();
    let mut outputs = common::Outputs::default();
    let result = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "inputs".into(),
            track: ConnectionTrack::Payload,
            bytes: input.encode().unwrap(),
        }],
        &mut outputs,
        &mut Vec::new(),
        &mut common::NoTimer,
    );
    assert!(result.is_err());
    assert_eq!(loads.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(outputs.0.is_empty());
}
