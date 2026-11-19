//! Owned-controller convenience adapter over independent endpoint custody.
//! Multiple endpoint contexts can be serviced by Root borrowing its one bus.
#![allow(dead_code)] // Existing owned adapter includes explicit Root retirement entrances.
pub(crate) use super::dwc2_endpoint_context::{Dwc2ReadRefusal, RootEndpointRoute};
use super::{
    dwc2_endpoint_context::Dwc2EndpointReadContext, endpoint_read_owner::EndpointReadCallOwner,
};
use crate::arch::dwc2::{Controller, RegisterIo};
use conduit_kernel::{HostCallId, NodeId, RequestId};
pub(crate) struct Dwc2EndpointReadHost<I> {
    controller: Controller<I>,
    context: Dwc2EndpointReadContext,
}
impl<I: RegisterIo> Dwc2EndpointReadHost<I> {
    /// # Safety
    /// Root owns this original controller and the exact admitted attachment,
    /// endpoint and generation. Keep this owner through physical retirement.
    pub unsafe fn bind_owned(
        controller: Controller<I>,
        owner: EndpointReadCallOwner,
        route: RootEndpointRoute,
    ) -> Result<Self, Dwc2ReadRefusal> {
        let context = unsafe { Dwc2EndpointReadContext::bind_admitted(&controller, owner, route) }?;
        Ok(Self {
            controller,
            context,
        })
    }
    pub fn execute(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request_id: RequestId,
        input: &[u8],
    ) -> Result<&[u8], Dwc2ReadRefusal> {
        // SAFETY: this private controller was bound with this context once;
        // the wrapper never replaces or exposes it, including after refusal.
        unsafe {
            self.context
                .execute(&mut self.controller, node, call, request_id, input)
        }
    }
    pub fn revoke(&mut self, node: NodeId, call: HostCallId) -> Result<(), Dwc2ReadRefusal> {
        self.context.revoke(node, call)
    }
    pub fn discard_retired(&mut self) -> Result<(), Dwc2ReadRefusal> {
        // SAFETY: the same private original controller owns every submission;
        // only its acknowledged quiescence can retire this context's lease.
        unsafe { self.context.discard_retired(&self.controller) }
    }
    pub fn retains_submission(&self) -> bool {
        self.context.retains_submission()
    }
}
