//! Fault injection at the provider boundary, exercised through ordinary Source/Plan/Play.
//! Timeout is a provider report here; this fixture does not enforce a wall deadline.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_ai::{ModelComputeOffer, ModelComputeRefusal};
use conduit_core::ConnectionTrack;
use conduit_data::TensorAxis;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_std_host::hosted_model_work::HostedModelWorkAdapter;
use conduit_std_host::{
    hosted_model_compute::*, hosted_model_infer::PreparedModelInfer, ExternalForeInput,
};
use fixture::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
enum Fault {
    ProviderLost,
    ProviderReportedTimeout,
}
struct FaultCompute {
    offer: ModelComputeOffer,
    fault: Fault,
    calls: Arc<AtomicUsize>,
}
impl ModelComputeAdapter for FaultCompute {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn load(&mut self, identity: [u8; 32], bytes: u64) -> Result<(), ModelComputeRefusal> {
        assert_ne!(identity, [0; 32]);
        assert_eq!(bytes, 24);
        Ok(())
    }
    fn execute(&mut self, _: ModelComputeInvocation<'_>) -> ModelComputeAdapterTerminal<'_> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.fault {
            Fault::ProviderLost => ModelComputeAdapterTerminal::ProviderLost,
            Fault::ProviderReportedTimeout => ModelComputeAdapterTerminal::TimedOut,
        }
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        Ok(())
    }
}

#[test]
fn source_kernel_preserves_shape_cancel_provider_loss_and_reported_timeout() {
    for (fault, cancel, wrong_shape, code, detail, expected_calls) in [
        (Fault::ProviderLost, false, false, "HostCallFailed", 71, 1),
        (
            Fault::ProviderReportedTimeout,
            false,
            false,
            "HostCallFailed",
            73,
            1,
        ),
        (Fault::ProviderLost, false, true, "InvalidInput", 72, 0),
        (Fault::ProviderLost, true, false, "Cancelled", 83, 0),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let retained_calls = calls.clone();
        let (mut host, plan) =
            host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
                let adapter = FaultCompute {
                    offer: offer(),
                    fault,
                    calls: retained_calls,
                };
                let mut prepared = PreparedModelInfer::prepare(
                    model(signature),
                    adapter,
                    requirement(),
                    "test/fault-injection@1".into(),
                )
                .unwrap();
                if cancel {
                    prepared.cancel();
                }
                Box::new(prepared)
            });
        let mut tensor = input([2., 3.]);
        if wrong_shape {
            tensor.dimensions = BoundedSequence::try_from_iter([1, 2]).unwrap();
            let axis: TensorAxis = tensor.axes[0].clone();
            tensor.axes = BoundedSequence::try_from_iter([axis.clone(), axis]).unwrap();
        }
        let mut outputs = Outputs::default();
        let result = host.run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "inputs".into(),
                track: ConnectionTrack::Payload,
                bytes: bundle(tensor).encode().unwrap(),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        );
        let error = match result {
            Err(error) => error,
            Ok(_) => panic!("injected terminal must not become successful inference"),
        };
        assert!(
            error.starts_with("installed kernel step: BackFailed"),
            "{error}"
        );
        assert!(error.contains(&format!("code: {code}")), "{error}");
        assert!(error.contains(&format!("detail: {detail}")), "{error}");
        assert!(outputs.0.is_empty());
        assert_eq!(calls.load(Ordering::SeqCst), expected_calls);
    }
}
