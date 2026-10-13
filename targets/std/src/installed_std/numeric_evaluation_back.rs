//! The hosted std executor delegates to the reviewed prepared numeric emitter.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::HostedValueStore;
pub(super) const IMPLEMENTATION: &str = "conduit.std/numeric-evaluation@1";
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: IMPLEMENTATION,
    budget,
    prepare,
};
fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let output = readmit(placement)?;
    let maximum = output.canonical().len() as u32;
    Ok(BackBudget {
        value_items: 1,
        value_bytes: maximum,
        host_requests: 0,
        sign_items: 0,
        maximum_value_bytes: maximum,
    })
}
fn readmit(
    placement: &PlannedGear,
) -> Result<conduit_core::numeric_realization::PreparedNumericOutput, String> {
    if placement.implementation_id.as_str() != IMPLEMENTATION {
        return Err("numeric std implementation mismatch".into());
    }
    conduit_semantic_catalog::prepare_numeric_evaluation(placement)
}
fn prepare(
    placement: &PlannedGear,
    _values: &mut HostedValueStore,
) -> Result<InstalledBack, String> {
    Ok(InstalledBack::NumericEvaluation(Box::new(
        conduit_semantic_catalog::NumericEvaluationBack::new(readmit(placement)?),
    )))
}
