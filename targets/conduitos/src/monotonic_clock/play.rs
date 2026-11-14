//! A reusable clock-only Play, driven by the containing machine's bounded quanta.
//! This owns no delay policy; callers submit exact absolute clock requests.
use super::{dispatch::PreparedClockDispatch, installation::*};
use crate::{
    protocol_host_calls::ProtocolCallRefusal, protocol_source::*,
    usb_base::hid_source_kernel::PreparedUsbSourceKernel,
};
use alloc::{collections::BTreeMap, vec, vec::Vec};
use conduit_core::*;
use conduit_monotonic_wait::clock_at::{contract::*, owner::*};

#[derive(Debug)]
pub enum ClockPlayRefusal {
    Binding,
    Source(ProtocolSourceRefusal),
    Installation(ClockInstallationRefusal),
    Issue(NativeProtocolIssueRefusal),
    Call(ProtocolCallRefusal),
    Value(StructuredInfoRefusal),
}
pub struct ClockPlay<P> {
    run: PreparedUsbSourceKernel,
    dispatch: PreparedClockDispatch<P>,
    input: PortId,
    output: PortId,
    request: ValuePayload,
    result: ValuePayload,
    composer: PreparedStructuredComposer,
    instant: PreparedStructuredComposer,
    sequence: u64,
    working: bool,
}
impl<P: MonotonicDeadlineProvider> ClockPlay<P> {
    pub fn prepare(
        ready: ReadyClockBase<P>,
        identity: &ClockNativeIdentity,
        issuer: NativeProtocolIssuer,
    ) -> Result<Self, ClockPlayRefusal> {
        use ClockPlayRefusal as E;
        let package = ProtocolSourcePackage::compile(
            "with machine/clock/at/request as ClockRequest\nwith machine/clock/at/result as ClockResult\nplot clock-call (\n >> requests: ClockRequest...|\n results: ClockResult...| >>\n) {\n clock: machine/clock/at\n requests >> clock >> results\n}\n".into(), &[]).map_err(E::Source)?;
        let bytes = serde_json::to_vec(&package).map_err(|_| E::Binding)?;
        let entry = PreparedProtocolEntry::prepare(&bytes, "clock-call").map_err(E::Source)?;
        let contract = MonotonicClockContract::prepare().map_err(E::Value)?;
        let mut host = HostAdvertisement {
            declared_name: None,
            protocol_version: PROTOCOL_VERSION,
            host_id: identity.host_id.clone(),
            boot_id: identity.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            profile: "conduitos/native-clock@1".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        ready
            .append_to_advertisement(&mut host, &contract)
            .map_err(E::Installation)?;
        entry.publish_pure_backs(&mut host).map_err(E::Source)?;
        let hosts = [host];
        let placements = entry.placements(&hosts).map_err(E::Source)?;
        let artifact = entry
            .plan(
                &hosts,
                &placements,
                &["conduit.base/local@1".into()],
                conduit_planner::PlanningOptions {
                    connection_bases: &BTreeMap::new(),
                    line_candidates: &BTreeMap::new(),
                    connection_item_capacity: 1,
                    connection_byte_capacity: CLOCK_MAXIMUM_BYTES,
                    authority_grants: core::slice::from_ref(issuer.grant()),
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .map_err(E::Source)?;
        let definition = artifact.artifact().definition();
        let plan = &definition.internal_plan;
        let fragment = &plan.fragments[0];
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let possession = issuer
            .issue(
                plan,
                &active,
                &CLOCK_IMPLEMENTATION.into(),
                MAXIMUM_POLL_STEPS,
            )
            .map_err(E::Issue)?;
        let dispatch = PreparedClockDispatch::prepare(
            plan,
            ready,
            possession.table,
            possession.handle,
            possession.claim,
        )
        .map_err(E::Call)?;
        let input = definition.boundary.input_fronts[0].external_port.clone();
        let output = definition.boundary.output_fronts[0].external_port.clone();
        let composer =
            PreparedStructuredComposer::new(contract.request_type(), CLOCK_MAXIMUM_BYTES as usize)
                .map_err(E::Value)?;
        let instant = PreparedStructuredComposer::new(
            &StructuredInfoType::leaf(kind_id("value/u64")).map_err(E::Value)?,
            64,
        )
        .map_err(E::Value)?;
        let run = PreparedUsbSourceKernel::prepare_clock(artifact).map_err(E::Call)?;
        Ok(Self {
            run,
            dispatch,
            input: input.port_id,
            output: output.port_id,
            request: ValuePayload {
                value_kind: input.value_kind,
                encoded: Vec::with_capacity(512),
            },
            result: ValuePayload {
                value_kind: output.value_kind,
                encoded: Vec::with_capacity(512),
            },
            composer,
            instant,
            sequence: 0,
            working: false,
        })
    }
    pub fn start(&mut self) -> Result<(), ClockPlayRefusal> {
        self.run
            .kernel_mut()
            .start()
            .map(|_| ())
            .map_err(|e| ClockPlayRefusal::Call(ProtocolCallRefusal::Kernel(e)))
    }
    pub fn submit(&mut self, deadline_ms: u64) -> Result<(), ClockPlayRefusal> {
        use ClockPlayRefusal as E;
        if self.working {
            return Err(E::Binding);
        }
        let next = self.sequence.checked_add(1).ok_or(E::Binding)?;
        let instant = self
            .instant
            .leaf(&deadline_ms.to_le_bytes())
            .map_err(E::Value)?;
        let value = self
            .composer
            .record(&[validate_canonical_structured_value(instant).map_err(E::Value)?])
            .map_err(E::Value)?;
        self.request.encoded.clear();
        self.request.encoded.extend_from_slice(value);
        let outcome = self
            .run
            .kernel_mut()
            .admit_input(&self.input, self.sequence, &self.request)
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        if !matches!(
            outcome,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err(E::Binding);
        }
        self.sequence = next;
        self.working = true;
        Ok(())
    }
    /// `true` means an actual admitted clock completion, never a simulated wake.
    pub fn step(&mut self) -> Result<bool, ClockPlayRefusal> {
        use ClockPlayRefusal as E;
        if !self.working {
            return Ok(false);
        }
        self.run
            .kernel_mut()
            .step()
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        if self.dispatch.has_pending() {
            self.dispatch.poll(self.run.kernel_mut()).map_err(E::Call)?;
        } else if let Some(request) = self.run.kernel_mut().next_host_request() {
            self.dispatch
                .dispatch(self.run.kernel_mut(), &request)
                .map_err(E::Call)?;
        }
        let Some(sequence) = self
            .run
            .kernel_mut()
            .output_into(&self.output, &mut self.result)
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?
        else {
            return Ok(false);
        };
        self.run
            .kernel_mut()
            .complete_output(&self.output, sequence)
            .map_err(|e| E::Call(ProtocolCallRefusal::Kernel(e)))?;
        self.working = false;
        if validate_canonical_structured_value(&self.result.encoded)
            .map_err(E::Value)?
            .variant_payload("completed")
            .map_err(E::Value)?
            .is_none()
        {
            return Err(E::Binding);
        }
        Ok(true)
    }
    pub fn pending(&self) -> bool {
        self.dispatch.has_pending()
    }
}

#[cfg(test)]
mod tests;
