//! Pending-provider custody tests; these are not native execution evidence.
#[path = "common_model_infer/mod.rs"]
mod fixture;
use conduit_ai::{ModelComputeOffer, ModelComputeRefusal, ModelComputeRuntimeIdentity};
use conduit_core::{PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use conduit_data::{NamedTensorBundle, PreparedNamedTensorBundleOutput};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_std_host::{
    hosted_model_compute::*,
    hosted_model_infer::PreparedModelInfer,
    hosted_model_work::{HostedModelWorkAdapter, ModelWorkTerminal},
};
use fixture::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

#[derive(Clone, Copy)]
enum Outcome {
    Valid,
    StaleRequest,
    WrongRuntime,
    Lost,
}
#[derive(Default)]
struct Counts {
    polls: AtomicUsize,
    cancels: AtomicUsize,
    unloads: AtomicUsize,
}
struct Deferred {
    offer: ModelComputeOffer,
    runtime: ModelComputeRuntimeIdentity,
    wrong_runtime: ModelComputeRuntimeIdentity,
    output: PreparedNamedTensorBundleOutput,
    request: [u8; 32],
    artifact: [u8; 32],
    inputs: [u8; 32],
    outcome: Outcome,
    counts: Arc<Counts>,
}
impl Deferred {
    fn new(outcome: Outcome, counts: Arc<Counts>) -> Self {
        let mut template = bundle(input([5., 1.]));
        // Semantic output port differs from the input port.
        let mut values = template.get().as_slice().to_vec();
        values[0].name = "output".into();
        template = NamedTensorBundle::from_values(values).unwrap();
        Self {
            offer: offer(),
            runtime: runtime("pending"),
            wrong_runtime: runtime("foreign"),
            output: PreparedNamedTensorBundleOutput::prepare(&template).unwrap(),
            request: [0; 32],
            artifact: [0; 32],
            inputs: [0; 32],
            outcome,
            counts,
        }
    }
}
impl ModelComputeAdapter for Deferred {
    fn offer(&self) -> &ModelComputeOffer {
        &self.offer
    }
    fn runtime_identity(&self) -> &ModelComputeRuntimeIdentity {
        &self.runtime
    }
    fn load(&mut self, _: [u8; 32], _: u64) -> Result<(), ModelComputeRefusal> {
        Ok(())
    }
    fn execute(
        &mut self,
        invocation: ModelComputeInvocation<'_>,
    ) -> ModelComputeAdapterTerminal<'_> {
        self.request = invocation.request_identity;
        self.artifact = invocation.artifact_identity;
        self.inputs = invocation.inputs.semantic_digest();
        ModelComputeAdapterTerminal::Pending
    }
    fn poll(&mut self) -> ModelComputeAdapterTerminal<'_> {
        if self.counts.polls.fetch_add(1, Ordering::SeqCst) == 0 {
            return ModelComputeAdapterTerminal::Pending;
        }
        if matches!(self.outcome, Outcome::Lost) {
            return ModelComputeAdapterTerminal::ProviderLost;
        }
        self.output.write("output", &[0; 8]).unwrap();
        let mut request = self.request;
        if matches!(self.outcome, Outcome::StaleRequest) {
            request[0] ^= 1;
        }
        ModelComputeAdapterTerminal::Produced(ModelComputeExecution {
            request_identity: request,
            artifact_identity: self.artifact,
            checkpoint_identity: None,
            inputs_identity: self.inputs,
            outputs: self.output.view().unwrap().encoded_ref(),
            consumed_work_units: 1,
            runtime: if matches!(self.outcome, Outcome::WrongRuntime) {
                &self.wrong_runtime
            } else {
                &self.runtime
            },
        })
    }
    fn cancel(&mut self) -> Result<ModelComputeAdapterTerminal<'_>, ModelComputeRefusal> {
        // Deliberately leave a producible result: wrapper revocation must fence it.
        self.counts.cancels.fetch_add(1, Ordering::SeqCst);
        Ok(ModelComputeAdapterTerminal::Cancelled)
    }
    fn unload(&mut self) -> Result<(), ModelComputeRefusal> {
        self.counts.unloads.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
fn prepared(outcome: Outcome) -> (PreparedModelInfer<Deferred>, PlannedGear, Arc<Counts>) {
    let (_, plan) = host_plan(include_str!("fixtures/model-linear.conduit"), |signature| {
        Box::new(
            PreparedModelInfer::prepare(
                model(signature),
                ReferenceModelComputeAdapter::new(offer(), runtime("pending")).unwrap(),
                requirement(),
                "test/pending@1".into(),
            )
            .unwrap(),
        )
    });
    let placement = plan.fragments[0].placements[0].clone();
    let signature =
        conduit_ai::model_infer_catalog::configured_model_signature(&placement.configuration)
            .unwrap();
    let counts = Arc::new(Counts::default());
    let value = PreparedModelInfer::prepare(
        model(signature),
        Deferred::new(outcome, counts.clone()),
        requirement(),
        "test/pending@1".into(),
    )
    .unwrap();
    (value, placement, counts)
}
#[test]
fn pending_retains_owned_input_and_publishes_only_after_matching_completion() {
    let (mut value, placement, _) = prepared(Outcome::Valid);
    let mut input = bundle(input([2., 3.])).encode().unwrap();
    let mut output = Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    assert_eq!(
        value.execute(&placement, &input, &mut output),
        ModelWorkTerminal::Pending
    );
    input.fill(0); // no input borrow survives execute
    assert_eq!(value.poll(&mut output), ModelWorkTerminal::Pending);
    assert!(output.is_empty());
    assert_eq!(value.poll(&mut output), ModelWorkTerminal::Produced);
    assert!(NamedTensorBundle::decode(&output).is_ok());
    assert_eq!(value.poll(&mut output), ModelWorkTerminal::Failed);
    assert!(output.is_empty());
}
#[test]
fn stale_completion_runtime_mismatch_and_provider_loss_cannot_publish() {
    for (outcome, expected) in [
        (Outcome::StaleRequest, ModelWorkTerminal::Failed),
        (Outcome::WrongRuntime, ModelWorkTerminal::Failed),
        (Outcome::Lost, ModelWorkTerminal::ProviderLost),
    ] {
        let (mut value, placement, _) = prepared(outcome);
        let input = bundle(input([2., 3.])).encode().unwrap();
        let mut output = Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES);
        assert_eq!(
            value.execute(&placement, &input, &mut output),
            ModelWorkTerminal::Pending
        );
        assert_eq!(
            value.execute(&placement, &input, &mut output),
            ModelWorkTerminal::Refused
        );
        assert_eq!(value.poll(&mut output), ModelWorkTerminal::Pending);
        assert_eq!(value.poll(&mut output), expected);
        assert!(output.is_empty());
    }
}
#[test]
fn cancellation_revokes_before_provider_and_never_polls_late_success() {
    let (mut value, placement, counts) = prepared(Outcome::Valid);
    let input = bundle(input([2., 3.])).encode().unwrap();
    let mut output = Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    assert_eq!(
        value.execute(&placement, &input, &mut output),
        ModelWorkTerminal::Pending
    );
    value.cancel();
    value.cancel();
    assert_eq!(value.poll(&mut output), ModelWorkTerminal::Cancelled);
    assert_eq!(
        value.execute(&placement, &input, &mut output),
        ModelWorkTerminal::Cancelled
    );
    assert!(output.is_empty());
    assert_eq!(counts.polls.load(Ordering::SeqCst), 0);
    assert_eq!(counts.cancels.load(Ordering::SeqCst), 1);
}
#[test]
fn explicit_retirement_and_drop_unload_once_and_fence_pending_results() {
    let (mut value, placement, counts) = prepared(Outcome::Valid);
    let input = bundle(input([2., 3.])).encode().unwrap();
    let mut output = Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    assert_eq!(
        value.execute(&placement, &input, &mut output),
        ModelWorkTerminal::Pending
    );
    value.unload();
    value.unload();
    assert_eq!(value.poll(&mut output), ModelWorkTerminal::ProviderLost);
    drop(value);
    assert_eq!(counts.unloads.load(Ordering::SeqCst), 1);
    let (value, _, counts) = prepared(Outcome::Valid);
    drop(value);
    assert_eq!(counts.unloads.load(Ordering::SeqCst), 1);
}
