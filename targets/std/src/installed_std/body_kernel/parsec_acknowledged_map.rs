//! Private selected scan state after exact issued Body Play and Fore acknowledgement.
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_core::{
    validate_canonical_structured_value, HostAdvertisement, PlannedActivationEntry,
    PlannedForePort, PreparationHostIdentity, StructuredCanonicalSelection, StructuredSelector,
};
use conduit_face::{
    ParsecMapTransitionResult, PARSEC_MAP_FRAME_BYTES, PARSEC_MAP_TRANSITION_SOURCE,
};
use conduit_kernel::NodeId;
use conduit_plan_lowering::lowering::{
    KernelForeEndpointIdentity, KernelIdentityMap, LoweredForePort,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedNativeType, StartupCatalog,
};

pub(super) struct ParsecAcknowledgedMap {
    prospective: BodyPlayIdentity,
    host: PreparationHostIdentity,
    port: PlannedForePort,
    endpoint: KernelForeEndpointIdentity,
    source_node: NodeId,
    maximum_items: u16,
    next_sequence: u64,
    issued: bool,
    revoked: bool,
    result: [u8; PARSEC_MAP_FRAME_BYTES],
    result_len: usize,
    state: Vec<u8>,
    candidate: Vec<u8>,
    selector: StructuredSelector,
    result_schema: Vec<u8>,
    state_schema: Vec<u8>,
    result_laws: CheckedNativeType,
}
impl ParsecAcknowledgedMap {
    pub(super) fn prepare(
        plan: &BodyPlan,
        host: &HostAdvertisement,
        prospective: &BodyPlayIdentity,
        identity: &KernelIdentityMap,
    ) -> Result<Self, String> {
        if !prospective.validate_for(plan) || plan.plots.len() != 1 {
            return Err(
                "Parsec map requires its exact prospective single-partition Body Play".into(),
            );
        }
        let parent = &plan.plots[0].plan;
        crate::BodyLiveForeQueue::validate_parsec_input_plan(parent, host)?;
        let PlannedActivationEntry::Scan(scan) = &parent.activations[0] else {
            return Err("Parsec map requires its selected scan".into());
        };
        let port = parent.fragments[0]
            .fore_ports
            .iter()
            .find(|port| port.front_port_id.as_str() == "states")
            .ok_or("Parsec map has no selected states Fore")?
            .clone();
        let fragment = &parent.fragments[0];
        if identity.plan_id != parent.plan_id || identity.fragment_id != fragment.fragment_id {
            return Err("Parsec map has a different lowered partition identity".into());
        }
        let source_node = identity
            .node_for_placement(&scan.owner_placement_id)
            .ok_or("Parsec scan owner has no exact lowered node")?;
        let endpoint = identity
            .fore_endpoints
            .iter()
            .find(|endpoint| {
                endpoint.front_port_id == port.front_port_id
                    && endpoint.direction == port.direction
                    && endpoint.track == port.track
                    && endpoint.value_kind == port.value_kind
            })
            .ok_or("Parsec states Fore has no exact lowered endpoint")?
            .clone();
        // Use exact authored native laws, not Rust copies of revision bounds.
        let checked = check_syntax_document(
            &parse_syntax_document(PARSEC_MAP_TRANSITION_SOURCE),
            &StartupCatalog::new(),
        )
        .map_err(|e| format!("Parsec map laws: {e:?}"))?;
        let laws = checked
            .native_types
            .iter()
            .find(|ty| ty.name == "ParsecMapTransitionResult")
            .ok_or("Parsec result owner metadata is absent")?
            .clone();
        if !laws.invariants.is_empty() {
            return Err("Parsec map whole-value laws require explicit prepared admission".into());
        }
        let native = ParsecMapTransitionResult::semantic_type().map_err(|e| format!("{e:?}"))?;
        if laws.value_type != native {
            return Err("Parsec result differs from authored owner".into());
        }
        let selector = StructuredSelector::field(native, "state").map_err(|e| format!("{e:?}"))?;
        let mut owner = Self {
            prospective: prospective.clone(),
            host: host_identity(host),
            port,
            endpoint,
            source_node,
            maximum_items: scan.limits.maximum_items,
            next_sequence: 0,
            issued: false,
            revoked: false,
            result: [0; PARSEC_MAP_FRAME_BYTES],
            result_len: 0,
            state: Vec::with_capacity(PARSEC_MAP_FRAME_BYTES),
            candidate: Vec::with_capacity(PARSEC_MAP_FRAME_BYTES),
            result_schema: selector
                .input_type()
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            state_schema: selector
                .output_type()
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            selector,
            result_laws: laws,
        };
        owner.stage(&scan.initial_accumulator)?;
        owner.commit(&scan.initial_accumulator);
        Ok(owner)
    }

    pub(super) fn confirm(
        &mut self,
        host: &HostAdvertisement,
        issued: &BodyPlayIdentity,
    ) -> Result<(), String> {
        if self.revoked
            || self.issued
            || issued != &self.prospective
            || self.host.host_id != host.host_id
            || self.host.boot_id != host.boot_id
            || self.host.offer_generation != host.offer_generation
        {
            self.revoke();
            return Err("Parsec map differs from actual issued Body Play or current Host".into());
        }
        self.issued = true;
        Ok(())
    }

    /// The Body runner calls this only after the exact selected scheduler Fore
    /// value has been successfully accepted and delivered, never in adapter code.
    pub(super) fn acknowledge_lowered(
        &mut self,
        port: &LoweredForePort,
        source_node: NodeId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<&[u8], String> {
        let result = self.acknowledge_inner(port, source_node, sequence, bytes);
        if result.is_err() {
            self.revoke();
        }
        result?;
        self.current_state()
    }
    fn acknowledge_inner(
        &mut self,
        port: &LoweredForePort,
        source_node: NodeId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<(), String> {
        if !self.issued
            || self.revoked
            || source_node != self.source_node
            || port.endpoint != self.endpoint.endpoint
            || port.cord != self.endpoint.cord
            || port.front_port_id != self.port.front_port_id
            || port.direction != self.port.direction
            || port.track != self.port.track
            || port.value_kind != self.port.value_kind
            || port.value_contract != self.port.value_contract
            || port.abnormal_kind != self.port.abnormal_kind
            || port.temporal != self.port.temporal
            || port.item_capacity != self.port.item_capacity
            || port.byte_capacity != self.port.byte_capacity
            || port.selected_line != self.port.selected_line
            || sequence != self.next_sequence
            || sequence >= u64::from(self.maximum_items)
        {
            return Err(
                "Parsec map acknowledgement differs from current selected Fore sequence".into(),
            );
        }
        self.stage(bytes)?;
        self.commit(bytes);
        self.next_sequence += 1;
        Ok(())
    }
    fn stage(&mut self, bytes: &[u8]) -> Result<(), String> {
        if bytes.len() > PARSEC_MAP_FRAME_BYTES {
            return Err("Parsec result exceeds admitted frame".into());
        }
        let value = validate_canonical_structured_value(bytes)
            .map_err(|e| format!("Parsec canonical result: {e:?}"))?;
        if value.type_bytes() != self.result_schema {
            return Err("Parsec result has wrong owner Type".into());
        }
        for law in &self.result_laws.value_contracts {
            let mut field = value;
            for name in law
                .representation_path
                .strip_prefix('.')
                .ok_or("Parsec law has nonrecord path")?
                .split('.')
            {
                field = field
                    .record_field(name)
                    .map_err(|e| format!("Parsec law field: {e:?}"))?
                    .ok_or("Parsec law field is absent")?;
            }
            let primitive = field
                .primitive_bytes(law.contract.value_kind.as_str())
                .map_err(|e| format!("Parsec law primitive: {e:?}"))?;
            law.contract
                .validate(primitive)
                .map_err(|e| format!("Parsec result law: {e:?}"))?;
        }
        if !matches!(
            self.selector.select_canonical_into(
                bytes,
                &self.result_schema,
                &self.state_schema,
                &mut self.candidate,
            ),
            Ok(StructuredCanonicalSelection::Matched)
        ) {
            return Err("Parsec selected state failed exact owner selection".into());
        }
        Ok(())
    }
    fn commit(&mut self, bytes: &[u8]) {
        self.result[..bytes.len()].copy_from_slice(bytes);
        self.result_len = bytes.len();
        core::mem::swap(&mut self.state, &mut self.candidate);
        self.candidate.clear();
    }
    pub(super) fn current_state(&self) -> Result<&[u8], String> {
        if !self.issued || self.revoked {
            return Err("Parsec map is unavailable".into());
        }
        Ok(&self.state)
    }
    /// Observation custody from this owner, never a caller-supplied Play ID.
    pub(super) fn acknowledged_items(&self) -> u64 {
        self.next_sequence
    }

    pub(super) fn current_play(&self) -> Result<&BodyPlayIdentity, String> {
        if !self.issued || self.revoked {
            return Err("Parsec map is unavailable".into());
        }
        Ok(&self.prospective)
    }
    #[cfg(test)]
    pub(super) fn current_result(&self) -> Result<&[u8], String> {
        if !self.issued || self.revoked {
            return Err("Parsec map is unavailable".into());
        }
        Ok(&self.result[..self.result_len])
    }
    pub(super) fn revoke(&mut self) {
        self.revoked = true;
        self.issued = false;
        self.result.fill(0);
        self.result_len = 0;
        self.state.fill(0);
        self.state.clear();
        self.candidate.fill(0);
        self.candidate.clear();
    }
}
impl Drop for ParsecAcknowledgedMap {
    fn drop(&mut self) {
        self.revoke();
    }
}

#[cfg(test)]
#[path = "parsec_acknowledged_map_tests.rs"]
mod tests;

fn host_identity(host: &HostAdvertisement) -> PreparationHostIdentity {
    PreparationHostIdentity {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
    }
}
