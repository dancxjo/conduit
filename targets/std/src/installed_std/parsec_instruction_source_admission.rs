//! Private post-Show Source admission with both parent and Mask scheduler custody.
use crate::flow_activation::{
    parsec_instruction_source_offer as source, ParsecInputInstructionSourcePlan,
};
use crate::installed_std::body_kernel::QualifiedParsecInstruction;
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_core::{ActivePlayIdentity, HostAdvertisement};
use conduit_face::{ParsecInputInstructionLook, PlannedMaskPlot, PreparedParsecInputInstruction};
use conduit_kernel::{
    scheduler::{FixedScheduler, HostCallRequest, StepBack},
    BoundedValueRef, HostCallDisposition, HostCallId, HostCallOutcome, NodeId, SignSink, ValueRef,
    ValueStorage,
};
use conduit_plan_lowering::lowering::{KernelIdentityMap, LoweredHostCall};
use sha2::{Digest, Sha256};

#[path = "parsec_instruction_source_completion.rs"]
mod completion;
pub(crate) use completion::CompletedParsecInstruction;

pub(crate) struct ParsecInstructionSourceAdmission {
    host: HostAdvertisement,
    parent: BodyPlan,
    parent_play: BodyPlayIdentity,
    mask: PlannedMaskPlot,
    mask_play: ActivePlayIdentity,
    operation: LoweredHostCall,
    issued: bool,
    accepted: Option<HostCallRequest>,
    completed: bool,
    revoked: bool,
    scope: [u8; 32],
    evaluator: PreparedParsecInputInstruction,
    wording: [u8; 256],
    wording_len: usize,
    producer: Option<(conduit_core::PlanId, conduit_core::PlotIdentity)>,
    completions: completion::InstructionCompletion,
}
impl ParsecInstructionSourceAdmission {
    /// Preparation alone does not accept a request or authorize an instruction.
    pub(crate) fn prepare(
        host: &HostAdvertisement,
        parent: &BodyPlan,
        prospective_parent: &BodyPlayIdentity,
        mask: &PlannedMaskPlot,
        prospective_mask: &ActivePlayIdentity,
        identity: &KernelIdentityMap,
        operations: &[LoweredHostCall],
    ) -> Result<Self, String> {
        let [fragment] = mask.plan.fragments.as_slice() else {
            return Err("instruction Source requires one exact local Mask fragment".into());
        };
        if !prospective_parent.validate_for(parent)
            || identity.plan_id != fragment.plan_id
            || identity.fragment_id != fragment.fragment_id
            || fragment.host_id != host.host_id
            || fragment.boot_id != host.boot_id
            || fragment.offer_generation != host.offer_generation
            || prospective_mask.plan_id != mask.plan.plan_id
            || prospective_mask.host_id != host.host_id
            || prospective_mask.boot_id != host.boot_id
            || *prospective_mask
                != conduit_core::bind_active_play(
                    &mask.plan.plan_id,
                    &host.host_id,
                    &host.boot_id,
                    prospective_mask.play_sequence,
                )
        {
            return Err(
                "instruction Source preparation has wrong parent/Mask/Host identity".into(),
            );
        }
        let mut selected = operations
            .iter()
            .filter(|op| op.contract_id.as_str() == source::CONTRACT);
        let operation = selected
            .next()
            .ok_or("Mask lacks lowered instruction Source call")?;
        if selected.next().is_some() {
            return Err("Mask has more than one instruction Source call".into());
        }
        let placement_id = identity
            .placement_for_node(operation.node)
            .ok_or("Source node lacks Mask placement")?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| &p.placement_id == placement_id)
            .ok_or("Source placement absent from selected Mask")?;
        source::validate_placement(placement)?;
        let requirement = source::operation()?;
        let expected_offer = source::offer()?;
        if host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == placement.capability_id)
            != Some(&expected_offer)
            || placement.host_id != host.host_id
            || placement.boot_id != host.boot_id
            || placement.offer_generation != host.offer_generation
            || operation.call != operation.binding.call
            || operation.call != HostCallId(0)
            || operation.target_kind != requirement.target_kind
            || operation.maximum_in_flight != requirement.maximum_in_flight
            || operation.binding.maximum_input_bytes != requirement.maximum_input_bytes
            || operation.binding.maximum_output_bytes != requirement.maximum_output_bytes
            || !identity.host_calls.iter().any(|(node, call, contract)| {
                *node == operation.node
                    && *call == operation.call
                    && contract == &operation.contract_id
            })
        {
            return Err(
                "instruction Source lacks exact current lowered Back/operation offer".into(),
            );
        }
        let evaluator = ParsecInputInstructionSourcePlan::prepare(host)?.evaluator()?;
        Ok(Self {
            host: host.clone(),
            parent: parent.clone(),
            parent_play: prospective_parent.clone(),
            mask: mask.clone(),
            mask_play: prospective_mask.clone(),
            operation: operation.clone(),
            issued: false,
            accepted: None,
            completed: false,
            revoked: false,
            scope: [0; 32],
            evaluator,
            wording: [0; 256],
            wording_len: 0,
            producer: None,
            completions: completion::InstructionCompletion::new(),
        })
    }
    /// Root calls only after actual parent and Mask Play issuance, never from descriptors.
    pub(crate) fn confirm_issued(
        &mut self,
        parent: &BodyPlayIdentity,
        mask: &ActivePlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        if self.revoked
            || self.issued
            || parent != &self.parent_play
            || mask != &self.mask_play
            || host != &self.host
        {
            self.revoke();
            return Err("instruction Source issued custody differs from preparation".into());
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.private/parsec-instruction-origin@1");
        for value in [
            parent.active_play_id.as_str(),
            mask.active_play_id.as_str(),
            mask.plan_id.as_str(),
        ] {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        }
        self.scope = hash.finalize().into();
        self.issued = true;
        Ok(())
    }
    pub(crate) fn accept(
        &mut self,
        request: HostCallRequest,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        if self.revoked
            || !self.issued
            || self.completed
            || self.accepted.is_some()
            || host != &self.host
            || request.node != self.operation.node
            || request.call != self.operation.call
            || request.request != conduit_kernel::RequestId(0)
            || request.input.admitted_bytes != source::MAXIMUM_SHOW_BYTES
            || request.input.value.byte_len == 0
            || request.input.value.byte_len > source::MAXIMUM_SHOW_BYTES
        {
            self.revoke();
            return Err(
                "instruction request is not the exact current Mask Source admission".into(),
            );
        }
        self.accepted = Some(request);
        Ok(())
    }
    /// Root revalidates/consumes the owner witness immediately before entering.
    /// Root must also match request.input to the actual acknowledged renderer Show
    /// ValueRef and bytes; identity/JSON matching alone does not establish that origin.
    pub(crate) fn complete_on_scheduler<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
    >(
        &mut self,
        request: HostCallRequest,
        qualified: &QualifiedParsecInstruction,
        look: ParsecInputInstructionLook,
        host: &HostAdvertisement,
        scheduler: &mut FixedScheduler<D, S, E, N, C, P, Q, R, T, H, K>,
    ) -> Result<ValueRef, String>
    where
        D: StepBack<P>,
        S: ValueStorage,
        E: SignSink,
    {
        let outcome = self.complete_inner(request, qualified, look, host, scheduler);
        if outcome.is_err() {
            self.revoke();
        }
        outcome
    }
    fn complete_inner<
        D,
        S,
        E,
        const N: usize,
        const C: usize,
        const P: usize,
        const Q: usize,
        const R: usize,
        const T: usize,
        const H: usize,
        const K: usize,
    >(
        &mut self,
        request: HostCallRequest,
        qualified: &QualifiedParsecInstruction,
        look: ParsecInputInstructionLook,
        host: &HostAdvertisement,
        scheduler: &mut FixedScheduler<D, S, E, N, C, P, Q, R, T, H, K>,
    ) -> Result<ValueRef, String>
    where
        D: StepBack<P>,
        S: ValueStorage,
        E: SignSink,
    {
        if self.revoked
            || !self.issued
            || self.completed
            || self.accepted != Some(request)
            || host != &self.host
        {
            return Err("instruction completion lacks accepted current Source request".into());
        }
        self.completions.require_capacity()?;
        qualified.require_source_scope(
            &self.parent,
            &self.parent_play,
            &self.mask,
            &self.mask_play,
            host,
        )?;
        let candidate = self
            .evaluator
            .evaluate(
                qualified.map_bytes(),
                qualified.binding_bytes(),
                look,
                false,
            )
            .map_err(|e| format!("instruction source evaluation: {e:?}"))?;
        let value = scheduler
            .store_host_value(candidate.canonical_bytes())
            .map_err(|e| format!("instruction storage: {e:?}"))?;
        let output = BoundedValueRef::new(value, source::MAXIMUM_INSTRUCTION_BYTES)
            .map_err(|e| format!("instruction bound: {e:?}"))?;
        if let Err(error) = scheduler.complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(output),
                failure: None,
            },
        ) {
            let _ = scheduler.discard_host_value(value);
            return Err(format!(
                "instruction scheduler completion refused: {error:?}"
            ));
        }
        self.producer = Some((
            candidate.producer_plan_id().clone(),
            candidate.producer_plot().clone(),
        ));
        self.wording_len = candidate.wording().len();
        self.wording[..self.wording_len].copy_from_slice(candidate.wording().as_bytes());
        self.completions.record(request, value, self.scope);
        self.completed = true;
        Ok(value)
    }
    pub(crate) fn claim(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<(), String> {
        self.completions.claim(node, call, value)
    }
    /// After exact internal Fore consumption/acknowledgement, transfer custody to
    /// the supplement effect request. This transfer itself acknowledges no effect.
    pub(crate) fn acknowledge(
        &mut self,
        node: NodeId,
        call: HostCallId,
        value: ValueRef,
    ) -> Result<CompletedParsecInstruction, String> {
        self.completions.acknowledge(node, call, value)
    }
    pub(crate) fn effect_producer(
        &self,
        completed: &CompletedParsecInstruction,
    ) -> Result<(&conduit_core::PlanId, &conduit_core::PlotIdentity), String> {
        self.completions
            .require_transferred(completed, self.scope)?;
        self.producer
            .as_ref()
            .map(|(plan, plot)| (plan, plot))
            .ok_or_else(|| "instruction completion has no checked producer custody".into())
    }
    pub(crate) fn effect_wording(
        &self,
        completed: &CompletedParsecInstruction,
    ) -> Result<&str, String> {
        self.completions
            .require_transferred(completed, self.scope)?;
        std::str::from_utf8(&self.wording[..self.wording_len])
            .map_err(|e| format!("instruction wording: {e}"))
    }
    pub(crate) fn require_effect_origin(
        &self,
        completed: &CompletedParsecInstruction,
        parent: &BodyPlayIdentity,
        mask: &ActivePlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        if self.revoked
            || !self.issued
            || parent != &self.parent_play
            || mask != &self.mask_play
            || host != &self.host
        {
            return Err("instruction effect parent/Mask/Host scope is stale".into());
        }
        self.completions.require_transferred(completed, self.scope)
    }
    /// Root calls only after its separate exact supplemental effect acknowledgement.
    pub(crate) fn consume_acknowledged_effect(
        &mut self,
        completed: CompletedParsecInstruction,
        parent: &BodyPlayIdentity,
        mask: &ActivePlayIdentity,
        host: &HostAdvertisement,
    ) -> Result<(), String> {
        self.require_effect_origin(&completed, parent, mask, host)?;
        self.completions.consume_effect(completed, self.scope)
    }
    pub(crate) fn revoke(&mut self) {
        self.revoked = true;
        self.accepted = None;
        self.completions.revoke();
    }
}
impl Drop for ParsecInstructionSourceAdmission {
    fn drop(&mut self) {
        self.revoke();
    }
}
