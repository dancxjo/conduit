//! One retained, typed Source stage through the ordinary pure protocol Play.
//! The containing machine forwards values; it owns no interpretation policy.
use super::*;
use crate::{
    protocol_host_calls::ProtocolCallRefusal, pure_protocol_play::PreparedPureProtocolPlay,
};
use alloc::collections::BTreeMap;
use conduit_core::*;
#[derive(Debug)]
pub enum PurePipeRefusal {
    Source(ProtocolSourceRefusal),
    Call(ProtocolCallRefusal),
    Binding,
    Pressure,
}
pub struct PreparedPureProtocolPipe {
    play: PreparedPureProtocolPlay,
    input: PortId,
    output: PortId,
    incoming: ValuePayload,
    result: ValuePayload,
    sequence: u64,
    working: bool,
    ready: bool,
    closed: bool,
    complete: bool,
}
impl PreparedPureProtocolPipe {
    pub fn prepare(
        package: ProtocolSourcePackage,
        entry_name: &str,
        host_id: HostId,
        boot_id: BootId,
    ) -> Result<Self, PurePipeRefusal> {
        Self::prepare_with_storage(package, entry_name, host_id, boot_id, Default::default())
    }
    pub(super) fn prepare_with_storage(
        package: ProtocolSourcePackage,
        entry_name: &str,
        host_id: HostId,
        boot_id: BootId,
        storage: conduit_composite::KernelCompositeSignStorage,
    ) -> Result<Self, PurePipeRefusal> {
        use PurePipeRefusal as E;
        let artifact = prepare_stage(package, entry_name, host_id, boot_id)?;
        crate::protocol_source::preparation_stage("pure-pipe-plan-ready");
        let boundary = &artifact.artifact().definition().boundary;
        let ([input], [output]) = (
            boundary.input_fronts.as_slice(),
            boundary.output_fronts.as_slice(),
        ) else {
            return Err(E::Binding);
        };
        let input = input.external_port.clone();
        let output = output.external_port.clone();
        crate::protocol_source::preparation_stage("pure-pipe-kernel");
        let play = artifact.prepare_pure(storage).map_err(E::Call)?;
        crate::protocol_source::preparation_stage("pure-pipe-ready");
        Ok(Self {
            play,
            input: input.port_id,
            output: output.port_id,
            incoming: ValuePayload {
                value_kind: input.value_kind,
                encoded: Vec::with_capacity(4096),
            },
            result: ValuePayload {
                value_kind: output.value_kind,
                encoded: Vec::with_capacity(4096),
            },
            sequence: 0,
            working: false,
            ready: false,
            closed: false,
            complete: false,
        })
    }
    pub fn start(&mut self) -> Result<(), PurePipeRefusal> {
        self.play.start().map_err(PurePipeRefusal::Call)
    }
    pub fn submit(&mut self, value: &ValuePayload) -> Result<(), PurePipeRefusal> {
        use PurePipeRefusal as E;
        if self.closed
            || self.working
            || self.ready
            || value.value_kind != self.incoming.value_kind
            || value.encoded.len() > 4096
        {
            return Err(E::Binding);
        }
        let next = self.sequence.checked_add(1).ok_or(E::Binding)?;
        self.incoming.encoded.clear();
        self.incoming.encoded.extend_from_slice(&value.encoded);
        if !matches!(
            self.play
                .admit_input(&self.input, self.sequence, &self.incoming)
                .map_err(E::Call)?,
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ) {
            return Err(E::Pressure);
        }
        self.sequence = next;
        self.working = true;
        Ok(())
    }
    pub fn step(&mut self) -> Result<(), PurePipeRefusal> {
        use PurePipeRefusal as E;
        let status = self.play.step().map_err(E::Call)?;
        if status == conduit_composite::KernelCompositeStatus::Cancelled {
            return Err(E::Binding);
        }
        if !self.ready {
            if let Some(sequence) = self
                .play
                .output_into(&self.output, &mut self.result)
                .map_err(E::Call)?
            {
                if !self.working {
                    return Err(E::Binding);
                }
                self.play
                    .complete_output(&self.output, sequence)
                    .map_err(E::Call)?;
                self.working = false;
                self.ready = true;
            }
        }
        self.complete = status == conduit_composite::KernelCompositeStatus::Complete;
        Ok(())
    }
    pub fn result(&self) -> Option<&ValuePayload> {
        self.ready.then_some(&self.result)
    }
    pub fn acknowledge(&mut self) {
        self.ready = false;
    }
    pub fn close(&mut self) -> Result<(), PurePipeRefusal> {
        if self.working || self.ready || self.closed {
            return Err(PurePipeRefusal::Binding);
        }
        self.play
            .close_input(&self.input)
            .map_err(PurePipeRefusal::Call)?;
        self.closed = true;
        Ok(())
    }
    pub fn complete(&self) -> bool {
        self.complete
    }
}

/// Prepare exact Source boundaries without adding interpretation or authority.
pub(super) fn prepare_stage(
    package: ProtocolSourcePackage,
    entry_name: &str,
    host_id: HostId,
    boot_id: BootId,
) -> Result<PreparedProtocolArtifact, PurePipeRefusal> {
    use PurePipeRefusal as E;
    let bytes = serde_json::to_vec(&package).map_err(|_| E::Binding)?;
    let entry = PreparedProtocolEntry::prepare(&bytes, entry_name).map_err(E::Source)?;
    let mut host = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation: OfferGeneration(1),
        profile: "conduitos/pure-source-pipe@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    crate::protocol_source::preparation_stage("pure-pipe-publish-backs");
    entry.publish_pure_backs(&mut host).map_err(E::Source)?;
    let hosts = [host];
    crate::protocol_source::preparation_stage("pure-pipe-place");
    let placements = entry.placements(&hosts).map_err(E::Source)?;
    crate::protocol_source::preparation_stage("pure-pipe-plan");
    let artifact = entry
        .plan(
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(E::Source)?;
    Ok(artifact)
}
