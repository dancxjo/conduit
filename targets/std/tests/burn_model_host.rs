#![cfg(feature = "burn-model")]
#[path = "../../../mechanisms/implementations/burn-model/tests/common/mod.rs"]
mod common;
use conduit_ai::{ModelComputeOperation, ModelComputeRequirement, PortableComputeClass};
use conduit_burn_model::{
    BurnAdapter, Cancellation, DeviceRequest, DirectoryCheckpointStore, InferenceBurnAdapter,
    InferenceContext, OptimizerRecipe,
};
use conduit_core::ComputeServiceGuarantee;
use conduit_std_host::hosted_model_compute::{
    HostedBurnModelComputeAdapter, ModelComputeAdapter, ModelComputeAdapterTerminal,
    ModelComputeInvocation,
};

#[test]
fn exported_burn_checkpoint_uses_existing_host_boundary() {
    let context = common::context();
    let artifact = context.artifact.content_identity();
    let inference_artifact = context.artifact.clone();
    let determinism_profile = context.realization.deterministic_profile.clone();
    let mut burn = BurnAdapter::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        context,
    )
    .unwrap();
    let cancel = Cancellation::default();
    let batch = common::batch();
    burn.train_step(&common::request(1), &batch, &cancel)
        .unwrap();
    let expected = burn.infer(&batch.inputs).unwrap().remove(0);
    let directory = tempfile::tempdir().unwrap();
    let store = DirectoryCheckpointStore::new(directory.path(), 65536, 16).unwrap();
    let checkpoint = burn.export_inference(&store, &cancel).unwrap();
    let checkpoint_id = checkpoint.content.identity.digest();
    let runtime = burn.runtime().clone();
    drop(burn);
    let inference = InferenceBurnAdapter::load(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        InferenceContext {
            artifact: inference_artifact,
            runtime,
            determinism_profile,
        },
        &store,
        &checkpoint_id,
    )
    .unwrap();
    let mut host = HostedBurnModelComputeAdapter::from_inference(inference).unwrap();
    host.load(artifact, 4096).unwrap();
    let request = ModelComputeInvocation {
        request_identity: [31; 32],
        artifact_identity: artifact,
        checkpoint_identity: Some(checkpoint_id),
        input: batch.inputs[0].clone(),
        requirement: ModelComputeRequirement {
            operation: ModelComputeOperation::Inference,
            model_format: "model/burn-safetensors@1".into(),
            element: conduit_data::TensorElement::F32,
            rank: 2,
            model_bytes: 4096,
            working_memory_bytes: 86016,
            device_memory_bytes: 0,
            input_bytes: 16,
            output_bytes: 32,
            batch_items: 4,
            compute_class: PortableComputeClass::GeneralCpu,
            minimum_lanes: 1,
            preferred_lanes: 1,
            maximum_lanes: 1,
            minimum_service: ComputeServiceGuarantee::Shared,
            solver_profile: None,
            determinism_profile: "seeded-order/f32-not-bitwise@1".into(),
            requires_checkpoint_load: false,
            requires_checkpoint_write: false,
        },
    };
    let offers = [host.offer().clone()];
    assert_eq!(
        conduit_ai::select_model_compute_offer(&offers, &request.requirement)
            .unwrap()
            .identity,
        host.offer().identity
    );
    let mut short = request.clone();
    short.requirement.output_bytes = 16;
    assert!(matches!(
        host.execute(short),
        ModelComputeAdapterTerminal::Refused(_)
    ));
    let mut too_few = request.clone();
    too_few.requirement.batch_items = 1;
    assert!(matches!(
        host.execute(too_few),
        ModelComputeAdapterTerminal::Refused(_)
    ));
    let mut stale = request.clone();
    stale.checkpoint_identity = Some([99; 32]);
    assert!(matches!(
        host.execute(stale),
        ModelComputeAdapterTerminal::Refused(_)
    ));
    let mut scarce = request.clone();
    scarce.requirement.working_memory_bytes = 1;
    assert!(matches!(
        host.execute(scarce),
        ModelComputeAdapterTerminal::Refused(_)
    ));
    let ModelComputeAdapterTerminal::Produced(output) = host.execute(request.clone()) else {
        panic!("exact checkpoint should produce");
    };
    assert_eq!(output.checkpoint_identity, Some(checkpoint_id));
    assert_eq!(output.output, expected);
    assert_eq!(output.runtime.device_evidence, "burn/flex-cpu");
    assert_eq!(output.consumed_work_units, 1000);
    host.unload().unwrap();
    assert!(matches!(
        host.execute(request),
        ModelComputeAdapterTerminal::ProviderLost
    ));
}
