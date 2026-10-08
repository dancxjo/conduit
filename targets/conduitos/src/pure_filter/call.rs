//! Exact native routing custody for the shared prepared Source selector.
use super::{PreparedPureFilter, PureFilterOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::{LoweredPlanFragment, lower_plan_fragment};
#[derive(Debug, PartialEq, Eq)]
pub enum FilterCallRefusal {
    WrongBinding,
    StaleRequest,
    SequenceExhausted,
    Cancelled,
    InvalidInput,
    Evaluation(conduit_plot::PortableExpressionEvaluationRefusal),
}
pub struct FilterHostCall {
    host: PreparedPureFilter,
    node: NodeId,
    next_request: u32,
    cancelled: bool,
    maximum_input_bytes: u32,
}
impl FilterHostCall {
    pub fn prepare(
        factory: &PureFilterOperationFactory,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        placement: &PlacementId,
    ) -> Result<Self, FilterCallRefusal> {
        let wrong = || FilterCallRefusal::WrongBinding;
        if fragment.plan_id != *factory.plan_id()
            || !verify_plan_fragment(fragment)
            || active.plan_id != fragment.plan_id
            || active.host_id != fragment.host_id
            || active.boot_id != fragment.boot_id
            || bind_active_play(
                &fragment.plan_id,
                &fragment.host_id,
                &fragment.boot_id,
                active.play_sequence,
            ) != *active
            || lower_plan_fragment(fragment).map_err(|_| wrong())? != *lowered
        {
            return Err(wrong());
        }
        let mut gears = fragment
            .placements
            .iter()
            .filter(|gear| &gear.placement_id == placement);
        let gear = gears.next().ok_or_else(wrong)?;
        if gears.next().is_some() {
            return Err(wrong());
        }
        if gear.host_id != fragment.host_id || gear.boot_id != fragment.boot_id {
            return Err(wrong());
        }
        let mut nodes = lowered
            .identity
            .placements
            .iter()
            .filter(|(_, id)| id == placement);
        let (node, _) = nodes.next().ok_or_else(wrong)?;
        if nodes.next().is_some() {
            return Err(wrong());
        }
        Ok(Self {
            host: factory.prepare_host(gear).map_err(|_| wrong())?,
            node: *node,
            next_request: 0,
            cancelled: false,
            maximum_input_bytes: gear.host_calls[0].maximum_input_bytes,
        })
    }
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<Option<&[u8]>, FilterCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(FilterCallRefusal::WrongBinding);
        }
        if self.cancelled {
            return Err(FilterCallRefusal::Cancelled);
        }
        if request != RequestId(self.next_request) {
            return Err(FilterCallRefusal::StaleRequest);
        }
        if input.len() > self.maximum_input_bytes as usize {
            return Err(FilterCallRefusal::InvalidInput);
        }
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(FilterCallRefusal::SequenceExhausted)?;
        self.host
            .execute(input)
            .map_err(FilterCallRefusal::Evaluation)
    }
    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), FilterCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(FilterCallRefusal::WrongBinding);
        }
        self.cancelled = true;
        Ok(())
    }
}
