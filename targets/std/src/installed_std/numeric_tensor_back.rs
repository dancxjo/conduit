//! Hosted TensorF64 projection keeps its checked coordinate custody in the Back.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{scheduler::StepBack, HostedValueStore, PortId};
pub(super) const IMPLEMENTATION: &str = "conduit.std/numeric-tensor@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn readmit(placement: &PlannedGear) -> Result<conduit_semantic_catalog::NumericTensorBack, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Err("numeric Tensor std implementation mismatch".into());
    }
    conduit_semantic_catalog::prepare_numeric_tensor(placement)
}
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let back = readmit(placement)?;
    let maximum = <conduit_semantic_catalog::NumericTensorBack as StepBack<1>>::prepared_output(
        &back,
        PortId(0),
    )
    .ok_or("Tensor prepared output missing")?
    .len() as u32;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: maximum,
        host_requests: 0,
        sign_items: 0,
        maximum_value_bytes: maximum,
    })
}
fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<InstalledBack, String> {
    Ok(InstalledBack::NumericTensor(Box::new(readmit(placement)?)))
}
