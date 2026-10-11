//! Std admission wrapper around the shared bounded kernel scan bridge.
use conduit_composite::{BoundedScanActivationHost, KernelScanBack};
use conduit_core::PlannedScanActivation;
pub(super) struct BodyScanBack;
impl BodyScanBack {
    pub(super) fn prepare(
        scan: BoundedScanActivationHost,
        planned: &PlannedScanActivation,
    ) -> Result<KernelScanBack, String> {
        let maximum_items =
            if planned.item_input.value_kind.as_str() == conduit_thermostat_plot::COMMAND_KIND {
                256
            } else {
                64
            };
        KernelScanBack::prepare(
            scan,
            planned,
            maximum_items,
            crate::flow_activation::maximum_scan_child_steps(planned)?,
        )
    }
}
