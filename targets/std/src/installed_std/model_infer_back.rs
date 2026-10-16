//! One named inference invocation through the existing model HostCall driver.
use super::back::{BackBudget, BackFactory, InstalledBack};
use crate::hosted_model_infer::*;
use conduit_core::PlannedGear;

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: MODEL_INFER_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) fn validate(placement: &PlannedGear) -> Result<(), String> {
    let kind = conduit_ai::model_infer_catalog::model_infer_contract()?;
    let signature =
        conduit_ai::model_infer_catalog::configured_model_signature(&placement.configuration)?;
    if placement.kind_id != kind.kind_id
        || placement.kind_contract_revision != kind.kind_contract_revision
        || placement.implementation_id.as_str() != MODEL_INFER_IMPLEMENTATION
        || placement.execution_profile_id.as_str() != MODEL_INFER_PROFILE
        || placement.inputs != kind.inputs
        || placement.outputs != kind.outputs
        || placement.configuration.len() != 1
        || placement.host_calls.len() != 1
    {
        return Err("planned model inference does not match installed Fore".into());
    }
    let call = &placement.host_calls[0];
    if call.contract_id.as_str() != MODEL_INFER_OPERATION
        || call.target_kind.as_ref() != Some(&kind.kind_id)
        || call.maximum_in_flight != 1
        || call.maximum_input_bytes != MODEL_INFER_MAXIMUM_BYTES
        || call.maximum_output_bytes != MODEL_INFER_MAXIMUM_BYTES
    {
        return Err("planned model inference HostCall exceeds installed contract".into());
    }
    conduit_ai::model_infer_catalog::signature_configuration(&signature)?;
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 4,
        value_bytes: MODEL_INFER_MAXIMUM_BYTES * 4,
        host_requests: 1,
        sign_items: 64,
        maximum_value_bytes: MODEL_INFER_MAXIMUM_BYTES,
    })
}
fn prepare(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::ModelWork(
        super::model_work_back::ModelWorkBack::inference(MODEL_INFER_MAXIMUM_BYTES),
    ))
}
