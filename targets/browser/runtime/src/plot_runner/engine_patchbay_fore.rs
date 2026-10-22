//! Resident input custody in the already admitted root kernel.
use super::{debug_error, TourScheduler};
use conduit_core::{PlanFragment, PortDirection, PortTemporal};
use conduit_face::MaskShow;
use conduit_kernel::{CordEndpoint, NodeId};
use conduit_plan_lowering::lowering::{LoweredForePort, LoweredPlanFragment};

pub(super) struct PatchbayFore {
    input: LoweredForePort,
    node: NodeId,
    plan_id: conduit_core::PlanId,
    host_id: conduit_core::HostId,
    boot_id: conduit_core::BootId,
    offer_generation: conduit_core::OfferGeneration,
    sequence: u64,
    uncertain: bool,
}

pub(super) fn prepare(
    partitions: &[(&PlanFragment, &LoweredPlanFragment)],
) -> Result<Vec<PatchbayFore>, String> {
    let mut result = Vec::new();
    for (fragment, lowered) in partitions {
        for placement in &fragment.placements {
            if placement.kind_id.as_str() != patchbay_application::PATCHBAY_INTERACTION_KIND {
                continue;
            }
            let node = lowered
                .nodes
                .iter()
                .find(|node| node.placement_id == placement.placement_id)
                .ok_or("resident Patchbay input owner is not lowered")?
                .node;
            let inputs = lowered
                .fore_ports
                .iter()
                .filter(|port| {
                    port.direction == PortDirection::Input
                        && lowered.cords.iter().any(|cord| {
                            cord.spec.cord == port.cord
                                && matches!(cord.spec.sink,
                        CordEndpoint::Local { node: sink, .. } if sink == node)
                        })
                })
                .collect::<Vec<_>>();
            let [input] = inputs.as_slice() else {
                return Err("resident Patchbay requires one exact local input Fore".into());
            };
            if input.value_kind.as_str() != conduit_face::FACE_INTERACTION_VALUE_KIND
                || input.temporal != (PortTemporal::Flow { closes: true })
                || input.selected_line.is_some()
                || input.item_capacity != 1
                || input.byte_capacity != conduit_face::MAX_FACE_INTERACTION_BYTES as u32
            {
                return Err("resident Patchbay input differs from its finite Fore".into());
            }
            result.push(PatchbayFore {
                input: (*input).clone(),
                node,
                plan_id: fragment.plan_id.clone(),
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
                sequence: 0,
                uncertain: false,
            });
        }
    }
    Ok(result)
}

impl TourScheduler {
    pub(in crate::plot_runner) fn has_resident_patchbay_input(&self) -> bool {
        !self.patchbay_fore.is_empty()
    }
    pub(in crate::plot_runner) fn has_patchbay_fore(&self, node: NodeId) -> bool {
        self.patchbay_fore.iter().any(|fore| fore.node == node)
    }

    pub(in crate::plot_runner) fn validate_patchbay_fore(
        &self,
        node: NodeId,
        show: &MaskShow,
    ) -> Result<(), String> {
        let fore = self
            .patchbay_fore
            .iter()
            .find(|fore| fore.node == node)
            .ok_or("no exact resident Patchbay input Fore")?;
        let fragment = fore;
        if fore.uncertain
            || fore.sequence >= u64::from(u32::MAX)
            || fragment.host_id != show.show.host_id
            || fragment.boot_id != show.show.boot_id
            || fragment.offer_generation != show.show.offer_generation
            || show.planned_mask.plan.fragments.iter().any(|mask| {
                mask.host_id != fragment.host_id
                    || mask.boot_id != fragment.boot_id
                    || mask.offer_generation != fragment.offer_generation
            })
        {
            return Err("resident Patchbay input is unavailable or belongs to another Host".into());
        }
        Ok(())
    }

    pub(in crate::plot_runner) fn admit_patchbay_fore(
        &mut self,
        node: NodeId,
        bytes: &[u8],
    ) -> Result<conduit_kernel::scheduler::HostCallRequest, String> {
        let fore = self
            .patchbay_fore
            .iter_mut()
            .find(|fore| fore.node == node)
            .ok_or("no exact resident Patchbay input Fore")?;
        if fore.uncertain || fore.sequence >= u64::from(u32::MAX) {
            return Err("resident Patchbay input sequence unavailable".into());
        }
        fore.input.validate_value(bytes).map_err(debug_error)?;
        match self
            .kernel
            .admit_remote_input(fore.input.endpoint, fore.input.cord, fore.sequence, bytes)
            .map_err(debug_error)?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. } => {
                fore.sequence += 1;
                fore.uncertain = true;
            }
            conduit_kernel::scheduler::RemoteIngressOutcome::Full { .. } => {
                return Err("resident Patchbay input is full".into())
            }
        }
        // Fair root scheduling, bounded by the installed node envelope. Select
        // only this owner's request; unrelated Host Calls remain undispatched.
        for _ in 0..super::MAXIMUM_BROWSER_GEARS * 4 {
            if let Some(call) = self
                .kernel
                .next_host_request_matching(|call| call.node == node)
            {
                return Ok(call);
            }
            match self.kernel.step().map_err(debug_error)? {
                conduit_kernel::scheduler::SchedulerStatus::Progress { .. } => {}
                status => {
                    return Err(format!(
                        "resident Patchbay input did not dispatch: {status:?}"
                    ))
                }
            }
        }
        Err("resident Patchbay input exceeded finite dispatch work".into())
    }

    pub(in crate::plot_runner) fn patchbay_input_plan(
        &self,
        node: NodeId,
    ) -> Result<conduit_core::PlanId, String> {
        Ok(self
            .patchbay_fore
            .iter()
            .find(|fore| fore.node == node)
            .ok_or("resident Patchbay input absent")?
            .plan_id
            .clone())
    }

    pub(in crate::plot_runner) fn complete_patchbay_fore(
        &mut self,
        call: conduit_kernel::scheduler::HostCallRequest,
        outcome: conduit_kernel::HostCallOutcome,
    ) -> Result<(), String> {
        let completed = outcome.disposition == conduit_kernel::HostCallDisposition::Completed;
        self.kernel
            .complete_host_call(call.node, call.request, outcome)
            .map_err(debug_error)?;
        // Release the actual call's retained input/completion before admitting
        // its successor. The one-slot input bound is not an extra retained value.
        for _ in 0..super::MAXIMUM_BROWSER_GEARS * 4 {
            if !self.kernel.has_retained_host_call_for_nodes(&[call.node]) {
                if completed {
                    self.patchbay_fore
                        .iter_mut()
                        .find(|fore| fore.node == call.node)
                        .ok_or("resident Patchbay input absent")?
                        .uncertain = false;
                }
                return Ok(());
            }
            match self.kernel.step().map_err(debug_error)? {
                conduit_kernel::scheduler::SchedulerStatus::Progress { .. } => {}
                status => {
                    return Err(format!(
                        "resident Patchbay completion did not settle: {status:?}"
                    ))
                }
            }
        }
        Err("resident Patchbay completion exceeded finite settlement work".into())
    }
}
