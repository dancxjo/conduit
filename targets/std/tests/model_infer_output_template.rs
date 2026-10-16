//! Provider-template custody at preparation and the existing Source Host Call.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_ai::{ModelComputeOffer, ModelComputeRefusal, ModelSignature};
use conduit_core::ConnectionTrack;
use conduit_data::NamedTensorBundle;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    hosted_model_compute::*,
    hosted_model_infer::{prepare_linear_model, PreparedModelInfer},
    ExternalForeInput,
};
use fixture::*;

struct FaultTemplate {
    inner: LinearF32ModelAdapter,
    unknown_port: bool,
}
impl FaultTemplate {
    fn new(signature: &ModelSignature, unknown_port: bool) -> Self {
        Self {
            inner: LinearF32ModelAdapter::new(
                offer(),
                runtime("fixture/template-drift"),
                [[2., 0.], [0., -1.]],
                [1., 4.],
            )
            .unwrap()
            .with_signature(signature)
            .unwrap(),
            unknown_port,
        }
    }
}
impl ModelComputeAdapter for FaultTemplate {
    fn offer(&self) -> &ModelComputeOffer {
        self.inner.offer()
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        self.inner.load(identity, bytes)
    }
    fn output_template(&self) -> Result<Option<NamedTensorBundle>, ModelComputeRefusal> {
        let template = self.inner.output_template()?.unwrap();
        let mut values = template.get().as_slice().to_vec();
        if self.unknown_port {
            values[0].name = "foreign-output".into();
        } else {
            // Signature permits an axis identity, but this profile's concrete
            // declaration must agree with the provider's actual output.
            let mut axes = values[0].value.axes.as_slice().to_vec();
            axes[0].identity = Some("declared-provider-axis".into());
            values[0].value.axes =
                conduit_plot::rust_binding::BoundedSequence::try_from_iter(axes).unwrap();
        }
        NamedTensorBundle::from_values(values)
            .map(Some)
            .map_err(|_| ModelComputeRefusal::UnsupportedShape)
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        self.inner.execute(invocation)
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        self.inner.cancel()
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.inner.unload()
    }
}

#[test]
fn provider_template_with_an_unknown_output_refuses_during_preparation() {
    let _ = host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
        let invalid = FaultTemplate::new(&signature, true);
        let result = PreparedModelInfer::prepare(
            model(signature.clone()),
            invalid,
            requirement(),
            "fixture/bad-template@1".into(),
        );
        assert!(result
            .err()
            .unwrap()
            .contains("output template differs from signature"));
        Box::new(
            prepare_linear_model(
                model(signature),
                offer(),
                runtime("reference-linear"),
                requirement(),
            )
            .unwrap(),
        )
    });
}

#[test]
fn actual_provider_axis_drift_cannot_be_replaced_by_template_metadata() {
    let (mut host, plan) = host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
        let adapter = FaultTemplate::new(&signature, false);
        Box::new(
            PreparedModelInfer::prepare(
                model(signature),
                adapter,
                requirement(),
                "fixture/template-axis-drift@1".into(),
            )
            .unwrap(),
        )
    });
    let mut outputs = Outputs::default();
    let result = host.run_external_plot_to(
        plan.fragments[0].clone(),
        &[ExternalForeInput {
            front_port_id: "inputs".into(),
            track: ConnectionTrack::Payload,
            bytes: bundle(input([2., 3.])).encode().unwrap(),
        }],
        &mut outputs,
        &mut Vec::new(),
        &mut NoTimer,
    );
    let Err(error) = result else {
        panic!("provider declaration mismatch must refuse output");
    };
    assert!(error.contains("HostCallFailed"), "{error}");
    assert!(outputs.0.is_empty());
}
