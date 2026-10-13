//! Prepare computational custody only from this Body runner's current offers.
use conduit_body::BodyPlotPlan;
use conduit_core::{HostAdvertisement, PlannedActivationEntry};
use std::sync::Arc;

pub(super) fn prepare(
    partitions: &[BodyPlotPlan],
    host: &HostAdvertisement,
) -> Result<Option<Arc<crate::flow_activation::ParsecSourcePlans>>, String> {
    if !partitions.iter().any(|partition| {
        partition
            .plan
            .activations
            .iter()
            .any(|entry| matches!(entry, PlannedActivationEntry::Scan(_)))
    }) {
        return Ok(None);
    }
    let command = crate::flow_activation::parsec_command_kind()?;
    let required = partitions.iter().any(|partition| {
        partition.plan.activations.iter().any(|entry| {
            matches!(entry, PlannedActivationEntry::Scan(scan) if scan.item_input.value_kind == command)
        })
    });
    required
        .then(|| crate::flow_activation::ParsecSourcePlans::prepare(host).map(Arc::new))
        .transpose()
}
