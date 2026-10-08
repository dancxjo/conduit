mod common;
use burn::{module::ParamGroup, tensor::Device};
use conduit_ai::{
    ModelComputeOperation, TrainStepOutcome, TrainingLifecyclePhase, TrainingObjective,
};
use conduit_burn_model::{
    AuthoringDescriptor, BurnAdapter, BurnModelDefinition, Cancellation, DeviceRequest, Error,
    ModelBatch, ObjectiveResult, OptimizerRecipe,
};
use conduit_data::TensorValue;

struct Definition {
    cancel: Option<Cancellation>,
    frozen: bool,
    candidate_bytes: Option<u64>,
}
impl BurnModelDefinition for Definition {
    type Model = common::Regression;
    fn descriptor(&self) -> AuthoringDescriptor {
        let mut descriptor = common::descriptor();
        descriptor.groups[0].trainable = !self.frozen;
        if let Some(bytes) = self.candidate_bytes {
            descriptor.resources.model_bytes = 8;
            descriptor.resources.candidate_bytes = bytes;
        }
        descriptor
    }
    fn initialize(&self, device: &Device) -> Result<Self::Model, Error> {
        common::RegressionDefinition.initialize(device)
    }
    fn parameter_group(&self, model: &Self::Model, id: &str) -> Result<ParamGroup, Error> {
        common::RegressionDefinition.parameter_group(model, id)
    }
    fn forward(
        &self,
        model: &Self::Model,
        inputs: &[TensorValue],
        device: &Device,
    ) -> Result<Vec<TensorValue>, Error> {
        let result = common::RegressionDefinition.forward(model, inputs, device)?;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        Ok(result)
    }
    fn objective(
        &self,
        model: &Self::Model,
        batch: &ModelBatch,
        objectives: &[TrainingObjective],
        device: &Device,
    ) -> Result<ObjectiveResult, Error> {
        let result = common::RegressionDefinition.objective(model, batch, objectives, device)?;
        if let Some(cancel) = &self.cancel {
            cancel.cancel();
        }
        Ok(result)
    }
}
fn adapter(definition: Definition) -> BurnAdapter<Definition> {
    BurnAdapter::initialize(
        definition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        common::context(),
    )
    .unwrap()
}
#[test]
fn cancellation_during_objective_discards_candidate() {
    let cancel = Cancellation::default();
    let mut host = adapter(Definition {
        cancel: Some(cancel.clone()),
        frozen: false,
        candidate_bytes: None,
    });
    let before = host.snapshot_identity().unwrap();
    let state = host.state().clone();
    assert!(matches!(
        host.train_step(&common::request(1), &common::batch(), &cancel)
            .unwrap(),
        TrainStepOutcome::NotCommitted { .. }
    ));
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(&state, host.state());
    assert_eq!(host.lifecycle().phase, TrainingLifecyclePhase::Cancelled);
}
#[test]
fn frozen_model_never_advertises_or_enters_training() {
    let mut host = adapter(Definition {
        cancel: None,
        frozen: true,
        candidate_bytes: None,
    });
    assert!(!host
        .offer()
        .supported_operations
        .contains(&ModelComputeOperation::TrainStep));
    let before = host.snapshot_identity().unwrap();
    assert!(host
        .train_step(
            &common::request(1),
            &common::batch(),
            &Cancellation::default()
        )
        .is_err());
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(host.lifecycle().phase, TrainingLifecyclePhase::Ready);
}
#[test]
fn stale_generation_and_insufficient_work_refuse_before_mutation() {
    let mut host = adapter(Definition {
        cancel: None,
        frozen: false,
        candidate_bytes: None,
    });
    let before = host.snapshot_identity().unwrap();
    let mut stale = common::request(1);
    stale.expected_generation = 1;
    assert!(host
        .train_step(&stale, &common::batch(), &Cancellation::default())
        .is_err());
    let mut scarce = common::request(1);
    scarce.admitted_work_units = 999;
    assert_eq!(
        host.train_step(&scarce, &common::batch(), &Cancellation::default()),
        Err(Error::ResourceBound)
    );
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(host.state().completed_steps, 0);
    host.unload().unwrap();
    assert_eq!(host.infer(&common::batch().inputs), Err(Error::Unloaded));
}
#[test]
fn candidate_storage_exhaustion_retains_committed_state() {
    let mut host = adapter(Definition {
        cancel: None,
        frozen: false,
        candidate_bytes: Some(8),
    });
    let before = host.snapshot_identity().unwrap();
    let state = host.state().clone();
    assert!(matches!(
        host.train_step(
            &common::request(1),
            &common::batch(),
            &Cancellation::default()
        )
        .unwrap(),
        TrainStepOutcome::NotCommitted {
            failure: conduit_ai::TrainStepFailure::ResourceExhausted,
            ..
        }
    ));
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(&state, host.state());
    assert_eq!(host.lifecycle().phase, TrainingLifecyclePhase::Failed);
}
#[cfg(feature = "cuda")]
#[test]
fn cpu_resume_in_cuda_build_refuses_ambient_optimizer_device() {
    let mut host = adapter(Definition {
        cancel: None,
        frozen: false,
        candidate_bytes: None,
    });
    let cancel = Cancellation::default();
    host.train_step(&common::request(1), &common::batch(), &cancel)
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let store = conduit_burn_model::DirectoryCheckpointStore::new(dir.path(), 65536, 16).unwrap();
    let checkpoint = host
        .checkpoint(
            &store,
            host.evaluate(&common::request(1).batch, &common::batch())
                .unwrap()
                .metrics,
            &cancel,
        )
        .unwrap();
    let before = host.snapshot_identity().unwrap();
    assert_eq!(
        host.resume(&store, &checkpoint.checkpoint.content.identity.digest()),
        Err(Error::UnsupportedResumeDevice)
    );
    assert_eq!(before, host.snapshot_identity().unwrap());
}

#[test]
fn finite_loss_with_nonfinite_gradient_never_commits_candidate() {
    use burn::tensor::Tensor;
    struct Singular(std::sync::Arc<std::sync::atomic::AtomicBool>);
    impl BurnModelDefinition for Singular {
        type Model = common::Regression;
        fn descriptor(&self) -> AuthoringDescriptor {
            common::descriptor()
        }
        fn initialize(&self, device: &Device) -> Result<Self::Model, Error> {
            common::RegressionDefinition.initialize(device)
        }
        fn parameter_group(&self, model: &Self::Model, id: &str) -> Result<ParamGroup, Error> {
            common::RegressionDefinition.parameter_group(model, id)
        }
        fn forward(
            &self,
            model: &Self::Model,
            inputs: &[TensorValue],
            device: &Device,
        ) -> Result<Vec<TensorValue>, Error> {
            common::RegressionDefinition.forward(model, inputs, device)
        }
        fn objective(
            &self,
            model: &Self::Model,
            batch: &ModelBatch,
            objectives: &[TrainingObjective],
            device: &Device,
        ) -> Result<ObjectiveResult, Error> {
            if !self.0.load(std::sync::atomic::Ordering::Acquire) {
                return common::RegressionDefinition.objective(model, batch, objectives, device);
            }
            // A finite zero loss with an infinite derivative, after AdamW has state.
            let prediction = model.linear.forward(Tensor::<2>::zeros([4, 1], device));
            let loss = (prediction.clone() - prediction.detach()).sqrt().mean();
            Ok(ObjectiveResult {
                loss,
                metrics: vec![conduit_ai::TrainingMetric::new(
                    conduit_ai::TrainingObjectiveIdentity::new("loss/mse".into()).unwrap(),
                    0,
                )
                .unwrap()],
            })
        }
    }
    let singular = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut host = BurnAdapter::initialize(
        Singular(singular.clone()),
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
        common::context(),
    )
    .unwrap();
    host.train_step(
        &common::request(1),
        &common::batch(),
        &Cancellation::default(),
    )
    .unwrap();
    singular.store(true, std::sync::atomic::Ordering::Release);
    let before = host.snapshot_identity().unwrap();
    let state = host.state().clone();
    assert!(matches!(
        host.train_step(
            &common::request(2),
            &common::batch(),
            &Cancellation::default()
        )
        .unwrap(),
        TrainStepOutcome::NotCommitted {
            failure: conduit_ai::TrainStepFailure::Failed,
            ..
        }
    ));
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(&state, host.state());
}

#[test]
fn cancellation_during_forward_discards_output_and_preserves_state() {
    let cancel = Cancellation::default();
    let mut host = adapter(Definition {
        cancel: Some(cancel.clone()),
        frozen: false,
        candidate_bytes: None,
    });
    let before = host.snapshot_identity().unwrap();
    let state = host.state().clone();
    assert_eq!(
        host.infer_cancellable(&common::batch().inputs, &cancel),
        Err(Error::Cancelled)
    );
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(&state, host.state());
}
