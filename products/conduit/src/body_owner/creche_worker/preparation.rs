//! Forward the actual successful adapter preparation, without issuing a Play.
use super::*;
use conduit_body_make::deployment_runtime::{CrecheOutcome, CrecheRequest};
use conduit_std_host::hosted_creche::{CrecheLimits, CrechePoll, PreparedCrecheRequest};
pub(super) struct Prepared {
    pub(super) context: CrecheExecutionContext,
    pub(super) request: CrecheRequest,
    pub(super) output: conduit_core::PortId,
}
pub(super) struct ObservedAdapter {
    inner: CrecheEncounterAdapter,
    prepared: Arc<OnceLock<Arc<Prepared>>>,
}
impl ObservedAdapter {
    pub(super) fn new(
        inner: CrecheEncounterAdapter,
        prepared: Arc<OnceLock<Arc<Prepared>>>,
    ) -> Self {
        Self { inner, prepared }
    }
    pub(super) fn into_inner(self) -> CrecheEncounterAdapter {
        self.inner
    }
}
impl HostedCrecheAdapter for ObservedAdapter {
    fn resource_offer(&self) -> conduit_core::ResourceOffer {
        self.inner.resource_offer()
    }
    fn limits(&self) -> CrecheLimits {
        self.inner.limits()
    }
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String> {
        if self.prepared.get().is_some() {
            return Err("Crèche preparation was already observed".into());
        }
        use conduit_body_make::deployment_runtime::{CRECHE_OUTCOME_BYTES, CRECHE_OUTCOME_KIND};
        use conduit_core::{kind_id, ConnectionTrack, PortDirection};
        let mut ports = context
            .plan()
            .plots
            .iter()
            .filter(|plot| &plot.plot == context.plot())
            .flat_map(|plot| &plot.plan.fragments)
            .filter(|fragment| &fragment.fragment_id == context.fragment_id())
            .flat_map(|fragment| &fragment.fore_ports)
            .filter(|port| {
                port.direction == PortDirection::Output
                    && port.placement_id == context.placement().placement_id
                    && port.value_kind == kind_id(CRECHE_OUTCOME_KIND)
                    && port.track == ConnectionTrack::Payload
            });
        let port = ports.next().ok_or("selected Crèche has no outcome Fore")?;
        if ports.next().is_some()
            || port.gear_port_id != conduit_core::port_id("outcome")
            || port.byte_capacity < CRECHE_OUTCOME_BYTES as u32
        {
            return Err("selected Crèche outcome Fore is ambiguous or insufficient".into());
        }
        let output = port.front_port_id.clone();
        let request = self.inner.prepare(context)?;
        let bound = context.bind_request(request.admitted_state_digest)?;
        self.prepared
            .set(Arc::new(Prepared {
                context: context.clone(),
                request: bound,
                output,
            }))
            .map_err(|_| "Crèche preparation observation is occupied")?;
        Ok(request)
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        self.inner.begin(request)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        self.inner.poll()
    }
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String> {
        self.inner.retained_evidence(outcome)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        self.inner.cancel_and_drain()
    }
}
