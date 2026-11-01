//! One scoped encounter owner beneath the exact kernel Host Call. This retains
//! correlation, never another queue, UI protocol or artifact truth.
use super::*;
use crate::hosted_creche::{CrecheHost, CrechePoll};
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_body_make::{deployment_runtime::*, deployment_runtime_catalog::*};
use conduit_core::HostAdvertisement;
use conduit_kernel::{scheduler::HostCallCancellation, HostCallId, NodeId, RequestId};

pub(super) struct CrecheRoute {
    node: NodeId,
    call: HostCallId,
    request: CrecheRequest,
    pending: Option<HostCallRequest>,
    dispatched: bool,
}
impl BodyKernel<'_> {
    pub(crate) fn attach_creche(
        &mut self,
        owner: &CrecheHost<'_>,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        if self.creche.is_some() {
            return Err("Creche owner is already attached".into());
        }
        let context = owner.context();
        if context.plan() != plan || context.play() != play || context.host() != host {
            return Err("Creche owner differs from actual producing Body/Plan/Play/Host".into());
        }
        if self.partitions.len() != plan.plots.len()
            || !self.partitions.iter().all(|identity| {
                plan.plots.iter().any(|plot| {
                    plot.plan.plan_id == identity.plan_id
                        && plot
                            .plan
                            .fragments
                            .iter()
                            .any(|fragment| fragment.fragment_id == identity.fragment_id)
                })
            })
        {
            return Err("Creche owner Body Plan differs from prepared kernel partitions".into());
        }
        let placement = context.placement();
        super::super::creche_back::validate(placement)?;
        let selected_resource = &placement.resources[0];
        let owned_resource = context.resource_offer();
        if selected_resource.pool_id != owned_resource.pool_id
            || selected_resource.class_id != owned_resource.class_id
            || selected_resource.units > owned_resource.capacity_units
            || selected_resource.content != owned_resource.content
        {
            return Err("Creche selected resource differs from exact owner custody".into());
        }
        let identity = self
            .partitions
            .iter()
            .find(|identity| &identity.fragment_id == context.fragment_id())
            .ok_or("Creche fragment was not lowered in this Body")?;
        let node = identity
            .node_for_placement(&placement.placement_id)
            .ok_or("Creche placement was not lowered in this Body")?;
        let mut operations = self
            .operations
            .iter()
            .filter(|op| op.contract_id.as_str() == CRECHE_HOST_CALL);
        let operation = operations
            .next()
            .ok_or("Creche owner has no planned Host Call")?;
        if operations.next().is_some()
            || operation.node != node
            || operation.call != HostCallId(0)
            || operation.target_kind.as_ref() != Some(&placement.kind_id)
            || operation.binding.maximum_input_bytes != CRECHE_REQUEST_BYTES as u32
            || operation.binding.maximum_output_bytes != CRECHE_OUTCOME_BYTES as u32
            || operation.maximum_in_flight != 1
        {
            return Err("Creche owner differs from its one exact lowered Host Call".into());
        }
        self.creche = Some(CrecheRoute {
            node,
            call: operation.call,
            request: owner.request(),
            pending: None,
            dispatched: false,
        });
        Ok(())
    }
    pub(super) fn creche_pending(&self) -> bool {
        self.creche
            .as_ref()
            .is_some_and(|route| route.pending.is_some())
    }
    pub(super) fn begin_creche(
        &mut self,
        request: HostCallRequest,
        owner: Option<&mut CrecheHost<'_>>,
    ) -> Result<(), String> {
        let owner = owner.ok_or("Creche Host Call has no scoped owner")?;
        let route = self
            .creche
            .as_mut()
            .ok_or("Creche Host Call was not admitted")?;
        if route.node != request.node
            || route.call != request.call
            || request.request != RequestId(0)
            || request.input.admitted_bytes != CRECHE_REQUEST_BYTES as u32
            || request.input.value.byte_len != CRECHE_REQUEST_BYTES as u32
            || route.dispatched
            || route.request != owner.request()
        {
            return Err("Creche Host Call differs from its exact admitted owner/request".into());
        }
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|e| format!("Creche request storage: {e:?}"))?;
        if input != route.request.0 {
            return Err("Creche request differs from selected admitted encounter".into());
        }
        route.pending = Some(request);
        route.dispatched = true;
        let result = owner.begin(route.request)?;
        self.complete_creche(result)
    }
    pub(super) fn poll_creche(
        &mut self,
        owner: Option<&mut CrecheHost<'_>>,
    ) -> Result<bool, String> {
        if !self.creche_pending() {
            return Ok(false);
        }
        let owner = owner.ok_or("pending Creche has no scoped owner")?;
        let result = owner.poll()?;
        let completed = matches!(result, CrechePoll::Outcome(_));
        self.complete_creche(result)?;
        Ok(completed)
    }
    fn complete_creche(&mut self, result: CrechePoll) -> Result<(), String> {
        let CrechePoll::Outcome(outcome) = result else {
            return Ok(());
        };
        let route = self
            .creche
            .as_mut()
            .ok_or("Creche completion has no admitted route")?;
        let request = route
            .pending
            .ok_or("Creche completion has no pending kernel request")?;
        if outcome.request != route.request {
            return Err("Creche completion belongs to another encounter".into());
        }
        // The owner validates retained canonical evidence before returning Outcome.
        let value = self
            .scheduler
            .store_host_value(&outcome.encode())
            .map_err(|e| format!("Creche outcome storage: {e:?}"))?;
        let output = BoundedValueRef::new(value, CRECHE_OUTCOME_BYTES as u32)
            .map_err(|e| format!("Creche outcome bound: {e:?}"))?;
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(|e| format!("Creche Host completion: {e:?}"))?;
        route.pending = None;
        Ok(())
    }
    pub(super) fn cancel_creche_call(
        &mut self,
        cancellation: HostCallCancellation,
        owner: Option<&mut CrecheHost<'_>>,
    ) -> Result<(), String> {
        let route = self
            .creche
            .as_mut()
            .ok_or("Creche cancellation has no owner route")?;
        let request = route
            .pending
            .ok_or("Creche cancellation has no pending request")?;
        if request.node != cancellation.node
            || request.call != cancellation.call
            || request.request != cancellation.request
        {
            return Err("Creche cancellation differs from exact pending request".into());
        }
        owner
            .ok_or("Creche cancellation has no scoped owner")?
            .retire()?;
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|e| format!("Creche cancellation completion: {e:?}"))?;
        route.pending = None;
        Ok(())
    }
}
