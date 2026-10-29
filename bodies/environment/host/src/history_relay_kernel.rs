//! One existing scheduler, lowered from the exact history provider fragment.
use crate::{checked, Result};
use conduit_environment_runtime::history_exchange::{HistoryHostCall, HistoryProvider};
use conduit_kernel::{
    self as k,
    scheduler::{FixedScheduler, HostCallBack},
    ValueStorage,
};
use conduit_plan_lowering::lowering::{
    LoweredPlanFragment, RemoteCordDirection, FIXED_KERNEL_STORAGE_PORTS_PER_NODE as PORTS,
};

type Kernel = FixedScheduler<
    HostCallBack,
    k::FixedValueStore<4, 9216>,
    k::FixedSignLog<4096>,
    1,
    2,
    PORTS,
    2,
    PORTS,
    2,
    1,
    1,
>;
pub(crate) struct HistoryKernel<P> {
    kernel: Kernel,
    owner: HistoryHostCall<P>,
    input: (k::RemoteEndpointId, k::CordId),
    output: (k::RemoteEndpointId, k::CordId),
    input_sequence: u64,
    pending: Option<k::scheduler::RemoteValueOffer>,
}
impl<P: HistoryProvider> HistoryKernel<P> {
    pub(crate) fn prepare(
        lowered: &LoweredPlanFragment,
        back: HostCallBack,
        owner: HistoryHostCall<P>,
    ) -> Result<Self> {
        if lowered.nodes.len() != 1
            || lowered.cords.len() != 2
            || lowered.host_calls.len() != 1
            || lowered.remote_endpoints.len() != 2
            || !lowered.states.is_empty()
            || !lowered.fusions.is_empty()
        {
            return Err("history provider requires exact one-node two-Cord fragment".into());
        }
        let endpoint = |direction| {
            lowered
                .remote_endpoints
                .iter()
                .find(|e| e.direction == direction)
                .map(|e| (e.endpoint, e.cord))
                .ok_or("history provider missing directional Cord")
        };
        let mut routes = k::FixedRoutes::<PORTS, 2>::new(PORTS as u16);
        for r in &lowered.routes {
            checked(routes.install(r.source_node, r.source_port, r.range, &r.targets))?;
        }
        checked(routes.seal())?;
        let mut calls = k::FixedHostCallBindings::<1>::new(1);
        let call = &lowered.host_calls[0];
        checked(calls.install(call.node, call.binding))?;
        checked(calls.seal())?;
        let signs = checked(k::FixedSignLog::<4096>::new_with_remote_storage(
            (4096 * std::mem::size_of::<k::KernelEvent>()) as u32,
            4096,
            k::remote_sign_storage_bytes(4096).ok_or("history remote Sign extent")?,
        ))?;
        let values = checked(k::FixedValueStore::<4, 9216>::new(9216))?;
        let kernel = checked(Kernel::new_with_host_calls(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "history node width")?,
            lowered
                .cords
                .iter()
                .map(|c| c.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "history Cord width")?,
            routes,
            calls,
            [back],
            values,
            signs,
        ))?;
        Ok(Self {
            kernel,
            owner,
            input: endpoint(RemoteCordDirection::Ingress)?,
            output: endpoint(RemoteCordDirection::Egress)?,
            input_sequence: 0,
            pending: None,
        })
    }
    pub(crate) fn dispatch(&mut self, input: &[u8]) -> Result<Vec<u8>> {
        if self.pending.is_some() {
            return Err("history output still awaiting exact delivery".into());
        }
        if !matches!(
            checked(self.kernel.admit_remote_input(
                self.input.0,
                self.input.1,
                self.input_sequence,
                input,
            ))?,
            k::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err("history input pressure preserves the request cursor".into());
        }
        self.input_sequence = self
            .input_sequence
            .checked_add(1)
            .ok_or("history input sequence exhausted")?;
        for _ in 0..64 {
            checked(self.kernel.step())?;
            if let Some(request) = self.kernel.next_host_request() {
                let input = checked(self.kernel.values().get(request.input.value))?;
                let result =
                    checked(
                        self.owner
                            .invoke(request.node, request.call, request.request, input),
                    )?;
                let value = checked(self.kernel.store_host_value(result))?;
                checked(self.kernel.complete_host_call(
                    request.node,
                    request.request,
                    k::HostCallOutcome {
                        disposition: k::HostCallDisposition::Completed,
                        output: Some(checked(k::BoundedValueRef::new(value, 2304))?),
                        failure: None,
                    },
                ))?;
            }
            if let Some(offer) = checked(
                self.kernel
                    .remote_egress_offer(self.output.0, self.output.1),
            )? {
                let bytes = checked(self.kernel.values().get(offer.value))?.to_vec();
                self.pending = Some(offer);
                return Ok(bytes);
            }
        }
        Err("history provider exceeded admitted dispatch steps".into())
    }
    pub(crate) fn cancel(&mut self) -> Result<()> {
        checked(self.kernel.cancel())?;
        checked(self.owner.revoke(k::NodeId(0), k::HostCallId(0)))?;
        while let Some(request) = self.kernel.next_host_cancellation() {
            checked(self.kernel.complete_host_call(
                request.node,
                request.request,
                k::HostCallOutcome {
                    disposition: k::HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                },
            ))?;
        }
        Ok(())
    }
    pub(crate) fn delivered(&mut self) -> Result<()> {
        let offer = self.pending.take().ok_or("history has no offered output")?;
        checked(
            self.kernel
                .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence),
        )?;
        checked(
            self.kernel
                .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence),
        )
    }
}
