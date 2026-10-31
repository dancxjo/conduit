//! One ordinary Host Call Back; file effects stay outside Step.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{scheduler::HostCallBack, HostedValueStore, NodeId};
pub(super) static PUBLISH: BackFactory = BackFactory {
    implementation_id: crate::linux_snapshot::PUBLISH,
    budget,
    prepare,
};
pub(super) static READ: BackFactory = BackFactory {
    implementation_id: crate::linux_snapshot::READ,
    budget,
    prepare,
};
fn validate(placement: &PlannedGear) -> Result<(), String> {
    let publish = match placement.implementation_id.as_str() {
        crate::linux_snapshot::PUBLISH => true,
        crate::linux_snapshot::READ => false,
        _ => return Err("snapshot implementation is not installed".into()),
    };
    let offer = crate::linux_snapshot::offer(publish);
    if placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || placement.limits != offer.limits
    {
        return Err("snapshot placement differs from installed realization".into());
    }
    conduit_linux_snapshot_store::PreparedSnapshotOperation::prepare(placement, NodeId(0))
        .map_err(|error| format!("snapshot preparation: {error:?}"))?;
    Ok(())
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    validate(placement)?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: 8192,
        host_requests: 1,
        sign_items: 16,
        maximum_value_bytes: 4096,
    })
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    validate(placement)?;
    Ok(InstalledBack::LinuxSnapshot(HostCallBack::new(
        placement.host_calls[0].maximum_input_bytes,
    )))
}
