mod common;
use conduit_ai::TrainStepOutcome;
use conduit_burn_model::{BurnAdapter, Cancellation, DeviceRequest, OptimizerRecipe};
fn adapter() -> BurnAdapter<common::RegressionDefinition> {
    BurnAdapter::initialize(
        common::RegressionDefinition,
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
fn burn_cpu_loss_decreases() {
    let mut host = adapter();
    let batch = common::batch();
    let before = *host
        .evaluate(&common::request(1).batch, &batch)
        .unwrap()
        .metrics[0]
        .value_millionths();
    let cancel = Cancellation::default();
    for step in 1..=80 {
        assert!(matches!(
            host.train_step(&common::request(step), &batch, &cancel)
                .unwrap(),
            TrainStepOutcome::Committed(_)
        ));
    }
    let after = *host
        .evaluate(&common::request(1).batch, &batch)
        .unwrap()
        .metrics[0]
        .value_millionths();
    println!("CPU regression: before_millionths={before}, after_millionths={after}, steps=80, precision=F32, device=burn/flex-cpu");
    assert!(after < before / 10, "before {before}, after {after}");
}
#[test]
fn evaluation_preserves_training_state() {
    let host = adapter();
    let before = host.snapshot_identity().unwrap();
    let state = host.state().clone();
    host.evaluate(&common::request(1).batch, &common::batch())
        .unwrap();
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(&state, host.state());
}
#[test]
fn cancelled_step_retains_parameters_and_optimizer() {
    let mut host = adapter();
    let before = host.snapshot_identity().unwrap();
    let cancel = Cancellation::default();
    cancel.cancel();
    assert!(matches!(
        host.train_step(&common::request(1), &common::batch(), &cancel)
            .unwrap(),
        TrainStepOutcome::NotCommitted { .. }
    ));
    assert_eq!(before, host.snapshot_identity().unwrap());
    assert_eq!(host.state().completed_steps, 0);
}
