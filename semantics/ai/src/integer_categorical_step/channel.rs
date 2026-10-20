//! Request custody around the existing production composite kernel.
//! Generic lease metadata is returned only with this private runtime's ACK.
//! This is numerical dispatch custody, not parser legality or Std-issued identity.
use super::{invocation::*, owner::CategoricalOperationFactory, *};
use alloc::collections::BTreeMap;
use conduit_composite::{
    KernelCompositeDefinition, KernelCompositeError, KernelCompositeHost, KernelCompositeStatus,
    KernelOperationRegistry,
};
use conduit_kernel::scheduler::RemoteIngressOutcome;
use conduit_plot::{CheckedPlot, CheckedSyntaxDocument, ProfileCatalog};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoricalChannelRefusal {
    Definition,
    Preparation,
    Busy,
    Pressure,
    Closed,
    Input,
    Sequence,
    Kernel,
    Evidence,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoricalChannelSubmission {
    Accepted,
    TerminalRetained(CategoricalChannelRefusal),
}
#[derive(Debug)]
pub struct CategoricalChannelSubmitFailure<L> {
    pub refusal: CategoricalChannelRefusal,
    pub lease: L,
}
struct Pending<L> {
    lease: L,
    token: Option<CommittedCategoricalInvocation>,
}
/// No constructor accepts an externally executed token or existing runtime.
/// `L` is metadata to AI; consumers must make their own lease unforgeable.
pub struct PreparedCategoricalChannel<L> {
    kernel: KernelCompositeHost,
    plays: Arc<BTreeMap<HostId, ActivePlayId>>,
    outlet: CategoricalInvocationOutlet,
    input_port: conduit_core::PortId,
    output_port: conduit_core::PortId,
    input: ValuePayload,
    input_codec: IntegerCollectionCodec,
    input_indices: Vec<u64>,
    output: ValuePayload,
    retained_input: Vec<u8>,
    retained_output: Vec<u8>,
    pending: Option<Pending<L>>,
    next: u64,
    closed: bool,
    last_kernel_error: Option<KernelCompositeError>,
}
/// Private construction ties the exact moved lease to this channel's invocation.
pub struct CompletedCategoricalRequest<L> {
    lease: L,
    invocation: DispatchedCategoricalInvocation,
}
/// Keeps actual runtime identity alongside the opaque numerical acknowledgement.
/// No public constructor or extraction of an uncorrelated owned token is exposed.
pub struct DispatchedCategoricalInvocation {
    invocation: CommittedCategoricalInvocation,
    plays: Arc<BTreeMap<HostId, ActivePlayId>>,
}
impl DispatchedCategoricalInvocation {
    pub fn active_plays(&self) -> &BTreeMap<HostId, ActivePlayId> {
        &self.plays
    }
}
impl core::ops::Deref for DispatchedCategoricalInvocation {
    type Target = CommittedCategoricalInvocation;
    fn deref(&self) -> &Self::Target {
        &self.invocation
    }
}
impl<L> CompletedCategoricalRequest<L> {
    pub fn into_parts(self) -> (L, DispatchedCategoricalInvocation) {
        (self.lease, self.invocation)
    }
}
impl<L> PreparedCategoricalChannel<L> {
    pub fn prepare(
        plot: &CheckedPlot,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        capacity: usize,
    ) -> Result<Self, CategoricalChannelRefusal> {
        use CategoricalChannelRefusal::*;
        let cap = &definition.external_capability;
        let rebuilt = KernelCompositeDefinition::from_authored_export(
            definition.host_id.clone(),
            definition.boot_id.clone(),
            definition.offer_generation,
            definition.profile.clone(),
            cap.implementation.implementation_id.clone(),
            cap.implementation.artifact_id.clone(),
            plot,
            &cap.capability_id,
            definition.internal_plan.clone(),
            definition.failure_translation.clone(),
        )
        .map_err(|_| Definition)?;
        if rebuilt != definition {
            return Err(Definition);
        }
        Self::prepare_verified(definition, profile, capacity)
    }
    /// Canonical Source and exact sealed Fore, preserving all three identities.
    pub fn prepare_canonical(
        document: &CheckedSyntaxDocument,
        entry: &str,
        catalog: &ProfileCatalog,
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        capacity: usize,
    ) -> Result<Self, CategoricalChannelRefusal> {
        use CategoricalChannelRefusal::Definition;
        let cap = &definition.external_capability;
        let rebuilt = KernelCompositeDefinition::from_canonical_export(
            definition.host_id.clone(),
            definition.boot_id.clone(),
            definition.offer_generation,
            definition.profile.clone(),
            cap.implementation.implementation_id.clone(),
            cap.implementation.artifact_id.clone(),
            document,
            entry,
            catalog,
            definition.internal_plan.clone(),
            definition.failure_translation.clone(),
        )
        .map_err(|_| Definition)?;
        if rebuilt != definition {
            return Err(Definition);
        }
        Self::prepare_verified(definition, profile, capacity)
    }
    fn prepare_verified(
        definition: KernelCompositeDefinition,
        profile: Arc<PreparedCategoricalStep>,
        capacity: usize,
    ) -> Result<Self, CategoricalChannelRefusal> {
        use CategoricalChannelRefusal::*;
        if definition.internal_plan.fragments.len() != 1
            || definition.internal_plan.fragments[0].placements.len() != 1
            || definition.boundary.input_fronts.len() != 1
            || definition.boundary.output_fronts.len() != 1
        {
            return Err(Definition);
        }
        let gear = &definition.internal_plan.fragments[0].placements[0];
        profile
            .verify_placement(gear, true)
            .map_err(|_| Definition)?;
        let input_front = &definition.boundary.input_fronts[0];
        let output_front = &definition.boundary.output_fronts[0];
        if input_front.internal_placement_id != gear.placement_id
            || output_front.internal_placement_id != gear.placement_id
            || input_front.internal_port_id != gear.inputs[0].port_id
            || output_front.internal_port_id != gear.outputs[0].port_id
        {
            return Err(Definition);
        }
        let input_port = input_front.external_port.port_id.clone();
        let output_port = output_front.external_port.port_id.clone();
        let input = ValuePayload {
            value_kind: input_front.external_port.value_kind.clone(),
            encoded: vec![0; profile.input_bytes as usize],
        };
        let output = ValuePayload {
            value_kind: output_front.external_port.value_kind.clone(),
            encoded: vec![0; profile.output_bytes as usize],
        };
        let (factory, mut outlets) = CategoricalOperationFactory::for_plan_with_invocation_outlets(
            &definition.internal_plan,
            &[profile.clone()],
            capacity,
        )
        .map_err(|_| Preparation)?;
        if outlets.len() != 1 {
            return Err(Definition);
        }
        let outlet = outlets.pop().ok_or(Definition)?;
        let mut registry = KernelOperationRegistry::new();
        registry.install(factory).map_err(|_| Preparation)?;
        let mut kernel =
            KernelCompositeHost::prepare(definition, &registry).map_err(|_| Preparation)?;
        kernel.start().map_err(|_| Preparation)?;
        let input_codec = IntegerCollectionCodec::prepare(&profile.indices, profile.dimensions().2)
            .map_err(|_| Preparation)?;
        let plays = Arc::new(kernel.active_plays().clone());
        Ok(Self {
            plays,
            input_codec,
            input_indices: vec![0; profile.dimensions().2],
            kernel,
            outlet,
            input_port,
            output_port,
            input,
            output,
            retained_input: vec![0; profile.input_bytes as usize],
            retained_output: vec![0; profile.output_bytes as usize],
            pending: None,
            next: 1,
            closed: false,
            last_kernel_error: None,
        })
    }
    pub fn active_plays(&self) -> &BTreeMap<HostId, ActivePlayId> {
        self.kernel.active_plays()
    }
    /// Allocating diagnostic snapshot, outside the bounded step path.
    pub fn signs(&self) -> BTreeMap<HostId, Vec<conduit_kernel::KernelEvent>> {
        self.kernel.signs()
    }
    pub fn pending(&self) -> Option<&L> {
        self.pending.as_ref().map(|pending| &pending.lease)
    }
    /// Most recent actual underlying error, retained without formatting/allocation.
    /// A later failed cancellation can replace the earlier error.
    pub fn last_kernel_error(&self) -> Option<&KernelCompositeError> {
        self.last_kernel_error.as_ref()
    }
    pub fn is_closed(&self) -> bool {
        self.closed
    }
    pub fn submit(
        &mut self,
        lease: L,
        exact_input: &[u8],
    ) -> Result<CategoricalChannelSubmission, CategoricalChannelSubmitFailure<L>> {
        use CategoricalChannelRefusal::*;
        let refusal = if self.closed {
            Some(Closed)
        } else if self.pending.is_some() {
            Some(Busy)
        } else if self.next == u64::MAX {
            Some(Sequence)
        } else if exact_input.len() != self.input.encoded.len() {
            Some(Input)
        } else if self
            .input_codec
            .decode(exact_input, &mut self.input_indices)
            .is_err()
        {
            Some(Input)
        } else if !self.outlet.has_capacity() {
            Some(Pressure)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            return Err(CategoricalChannelSubmitFailure { refusal, lease });
        }
        self.input.encoded.copy_from_slice(exact_input);
        // Own the lease before crossing actual ingress. No other kernel writer exists.
        self.pending = Some(Pending { lease, token: None });
        match self
            .kernel
            .admit_input(&self.input_port, self.next - 1, &self.input)
        {
            Ok(RemoteIngressOutcome::Accepted { sequence }) if sequence == self.next - 1 => {
                Ok(CategoricalChannelSubmission::Accepted)
            }
            Ok(RemoteIngressOutcome::Full { .. }) => {
                let pending = self.pending.take().expect("private pending before ingress");
                Err(CategoricalChannelSubmitFailure {
                    refusal: Pressure,
                    lease: pending.lease,
                })
            }
            Err(error) => {
                self.last_kernel_error = Some(error);
                self.closed = true;
                Ok(CategoricalChannelSubmission::TerminalRetained(Kernel))
            }
            _ => {
                // An unexpected admission outcome might have accepted an effect.
                // Keep its lease privately rather than returning it for reuse.
                self.closed = true;
                Ok(CategoricalChannelSubmission::TerminalRetained(Kernel))
            }
        }
    }
    /// Exactly one production kernel step. No busy loop or caller observation callback.
    pub fn step(
        &mut self,
    ) -> Result<Option<CompletedCategoricalRequest<L>>, CategoricalChannelRefusal> {
        use CategoricalChannelRefusal::*;
        if self.closed {
            return Err(Closed);
        }
        if self.pending.is_none() {
            return Ok(None);
        }
        match self.kernel.step() {
            Ok(KernelCompositeStatus::Active) => (),
            Err(error) => {
                self.last_kernel_error = Some(error);
                self.closed = true;
                return Err(Kernel);
            }
            _ => {
                self.closed = true;
                return Err(Kernel);
            }
        }
        let pending = self.pending.as_mut().ok_or(Evidence)?;
        if pending.token.is_none() {
            pending.token = self.outlet.try_take();
        }
        let Some(token) = pending.token.as_ref() else {
            return Ok(None);
        };
        if !self.outlet.contains(token)
            || token.invocation() != self.next
            || token.copy_input_into(&mut self.retained_input).is_err()
            || self.retained_input != self.input.encoded
            || token.copy_output_into(&mut self.retained_output).is_err()
        {
            self.closed = true;
            return Err(Evidence);
        }
        let sequence = match self.kernel.output_into(&self.output_port, &mut self.output) {
            Ok(Some(sequence)) => sequence,
            Ok(None) => return Ok(None),
            Err(error) => {
                self.last_kernel_error = Some(error);
                self.closed = true;
                return Err(Kernel);
            }
        };
        if sequence != self.next - 1 || self.output.encoded != self.retained_output {
            self.closed = true;
            return Err(Evidence);
        }
        if let Err(error) = self.kernel.complete_output(&self.output_port, sequence) {
            self.last_kernel_error = Some(error);
            self.closed = true;
            return Err(Kernel);
        }
        self.next += 1;
        let pending = self.pending.take().ok_or(Evidence)?;
        Ok(Some(CompletedCategoricalRequest {
            lease: pending.lease,
            invocation: DispatchedCategoricalInvocation {
                invocation: pending.token.ok_or(Evidence)?,
                plays: self.plays.clone(),
            },
        }))
    }
    /// Terminal even when cancellation reports a kernel error. Pending evidence stays owned.
    pub fn cancel(&mut self) -> Result<(), CategoricalChannelRefusal> {
        self.closed = true;
        match self.kernel.cancel() {
            Ok(()) => Ok(()),
            Err(error) => {
                self.last_kernel_error = Some(error);
                Err(CategoricalChannelRefusal::Kernel)
            }
        }
    }
    /// Recover exact pending custody only by consuming the terminal channel.
    pub fn into_pending(
        mut self,
    ) -> Result<Option<(L, Option<CommittedCategoricalInvocation>)>, Self> {
        if !self.closed {
            return Err(self);
        }
        if let Some(pending) = &mut self.pending {
            if pending.token.is_none() {
                pending.token = self.outlet.try_take();
            }
        }
        Ok(self.pending.map(|pending| (pending.lease, pending.token)))
    }
}
