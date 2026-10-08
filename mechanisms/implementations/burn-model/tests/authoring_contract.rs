#[allow(dead_code)]
mod common;
use conduit_burn_model::{ParameterGroup, ResourceEstimate};

#[test]
fn authoring_contract_refuses_invalid_profiles() {
    let mut descriptor = common::descriptor();
    assert!(descriptor.validate().is_ok());
    descriptor.architecture.clear();
    assert!(descriptor.validate().is_err());
    descriptor = common::descriptor();
    descriptor.groups.push(ParameterGroup {
        identity: "all".into(),
        trainable: false,
    });
    assert!(descriptor.validate().is_err());
    descriptor = common::descriptor();
    descriptor.resources = ResourceEstimate {
        model_bytes: u64::MAX,
        candidate_bytes: u64::MAX,
        ..descriptor.resources
    };
    assert!(descriptor.validate().is_err());
}

#[test]
fn fresh_initialization_refuses_unrelated_base_weights() {
    use conduit_burn_model::{
        BurnAdapter, DeviceRequest, Error, OptimizerRecipe, PreparedBurnModel,
    };
    let prepared = PreparedBurnModel::initialize(
        common::RegressionDefinition,
        DeviceRequest::Cpu,
        OptimizerRecipe {
            learning_rate: 0.05,
            weight_decay: 0.,
            gradient_clip: 10.,
            seed: 42,
        },
    )
    .unwrap();
    let mut context = common::context();
    context.artifact.content.identity =
        conduit_core::ResourceSemanticIdentity::from_digest([99; 32]);
    context.session.base_artifact_identity = [99; 32];
    assert!(matches!(
        BurnAdapter::from_prepared(prepared, context),
        Err(Error::IncompatibleCheckpoint)
    ));
}
