//! Budgeted binding is needed only by the finite appliance execution fixture.
use super::pulse::{bind_mode, Provider};
use conduit_core::Plan;
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use conduit_pulse_capture::{
    installation::ReadyPulseBase,
    owner::{PulseCallRefusal, PulseHostCall},
};

/// Explicit finite CPU Root budget, independent of received assignment bytes.
pub fn bind_budget(
    ready: ReadyPulseBase<Provider>,
    plan: &Plan,
    lowered: &LoweredPlanFragment,
    operations: u32,
) -> Result<(PulseHostCall<Provider>, NodeId), PulseCallRefusal> {
    bind_mode(ready, plan, lowered, None, operations)
}
