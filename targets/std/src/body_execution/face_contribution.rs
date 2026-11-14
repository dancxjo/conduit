//! One bounded, kernel-committed semantic contribution. Observation grants no
//! current Show, input authority, durable recording, or producer authentication.
use conduit_body::{BodyPlan, BodyPlayIdentity, Wake};
use conduit_core as c;
use conduit_face::{FaceContributionBasis, FaceFragment};
use conduit_kernel::{
    scheduler::{FixedScheduler, HostCallRequest, StepBack, StepIo},
    HostCallDisposition, HostCallId, NodeId, RequestId, SignSink, ValueStorage,
};
use conduit_plan_lowering::lowering::KernelIdentityMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaceContributionRefusal {
    Scope,
    Bounds,
    Pressure,
    Completion,
    Revoked,
}
/// An immutable observation, not constructible/deserializable from Face bytes.
/// Historical scope remains inspectable; the receiving owner must still check
/// its current Body incarnation, realization and workset before projection.
#[derive(Debug)]
pub struct CommittedFaceContribution {
    scope: Arc<FaceContributionScope>,
    request: RequestId,
    fragment: FaceFragment,
}
#[derive(Debug)]
pub struct FaceContributionScope {
    body_play: BodyPlayIdentity,
    host: c::HostId,
    boot: c::BootId,
    plot: c::PlotIdentity,
    partition_plan: c::PlanId,
    fragment: c::FragmentId,
    placement: c::PlacementId,
    node: NodeId,
    call: HostCallId,
    maximum_input_bytes: u32,
    lifetime: Arc<CommitState>,
    _memory: Option<crate::kernel_preparation::ImageMemoryLease>,
}
impl CommittedFaceContribution {
    pub fn scope(&self) -> &FaceContributionScope {
        &self.scope
    }
    pub fn request(&self) -> RequestId {
        self.request
    }
    pub fn fragment(&self) -> &FaceFragment {
        &self.fragment
    }
}
impl FaceContributionScope {
    /// A shared, actual Back lifetime witness. This is not a substitute for
    /// the receiving owner's current Body/Host/Boot/Plan/Play checks.
    pub fn is_retired(&self) -> bool {
        self.lifetime.revoked.load(Ordering::Acquire)
    }

    pub fn body_play(&self) -> &BodyPlayIdentity {
        &self.body_play
    }
    pub fn host(&self) -> &c::HostId {
        &self.host
    }
    pub fn boot(&self) -> &c::BootId {
        &self.boot
    }
    pub fn plot(&self) -> &c::PlotIdentity {
        &self.plot
    }
    pub fn partition_plan(&self) -> &c::PlanId {
        &self.partition_plan
    }
    pub fn fragment(&self) -> &c::FragmentId {
        &self.fragment
    }
    pub fn placement(&self) -> &c::PlacementId {
        &self.placement
    }
    pub fn node(&self) -> NodeId {
        self.node
    }
    pub fn call(&self) -> HostCallId {
        self.call
    }
}
/// A refusal returns the original observation, retaining its finite ownership.
#[derive(Debug)]
pub struct ReturnedFaceContribution {
    pub contribution: CommittedFaceContribution,
    pub refusal: FaceContributionRefusal,
}
pub trait BodyFaceContributionObserver {
    // Refusal returns original charged custody; allocating a Box under pressure
    // would add storage exactly when the consumer cannot accept ownership.
    #[allow(clippy::result_large_err)]
    fn contribute(
        &mut self,
        contribution: CommittedFaceContribution,
    ) -> Result<(), ReturnedFaceContribution>;
}

#[derive(Debug)]
struct CommitState {
    placement: c::PlacementId,
    call: HostCallId,
    committed: AtomicU64,
    attached: AtomicBool,
    revoked: AtomicBool,
}
/// This gate belongs inside the real installed Back. Each Step must call
/// begin_step; consume_completion stages its exact consumption; ONLY that
/// Back's kernel-invoked step_committed may call step_committed here.
/// The hook uses bounded atomics: no allocation, lock, channel or Face work.
pub(crate) struct FaceCommitGate {
    state: Arc<CommitState>,
    candidate: Option<RequestId>,
    commits: u32,
    maximum_commits: u32,
}
pub(crate) struct FaceCommitObservation(Arc<CommitState>);
impl FaceCommitGate {
    pub(crate) fn prepare(
        placement: c::PlacementId,
        call: HostCallId,
        maximum_commits: u32,
    ) -> Result<Self, FaceContributionRefusal> {
        if placement.as_str().is_empty() || maximum_commits == 0 {
            return Err(FaceContributionRefusal::Bounds);
        }
        Ok(Self {
            state: Arc::new(CommitState {
                placement,
                call,
                committed: AtomicU64::new(0),
                attached: AtomicBool::new(false),
                revoked: AtomicBool::new(false),
            }),
            candidate: None,
            commits: 0,
            maximum_commits,
        })
    }
    pub(crate) fn prepared_backing_bytes(&self) -> Option<usize> {
        core::mem::size_of::<CommitState>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(self.state.placement.0.capacity())
    }
    pub(crate) fn observation(&self) -> FaceCommitObservation {
        FaceCommitObservation(self.state.clone())
    }
    pub(crate) fn begin_step(&mut self) {
        self.candidate = None;
    }
    pub(crate) fn consume_completion<const PORTS: usize>(
        &mut self,
        io: &mut StepIo<PORTS>,
        expected: RequestId,
    ) -> Result<(), FaceContributionRefusal> {
        if self.state.revoked.load(Ordering::Acquire) {
            return Err(FaceContributionRefusal::Revoked);
        }
        if self.commits == self.maximum_commits {
            return Err(FaceContributionRefusal::Bounds);
        }
        let (request, outcome) = io
            .host_completion()
            .ok_or(FaceContributionRefusal::Completion)?;
        if request != expected
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
        {
            return Err(FaceContributionRefusal::Completion);
        }
        io.consume_host_completion()
            .map_err(|_| FaceContributionRefusal::Completion)?;
        self.candidate = Some(request);
        Ok(())
    }
    pub(crate) fn step_committed(&mut self) {
        if let Some(request) = self.candidate.take() {
            self.commits += 1;
            self.state
                .committed
                .store(u64::from(request.0) + 1, Ordering::Release);
        }
    }
    pub(crate) fn cancel(&mut self) {
        self.candidate = None;
        self.state.revoked.store(true, Ordering::Release);
    }
}

pub(crate) struct FaceContributionContext<'a> {
    pub wake: &'a Wake,
    pub plan: &'a BodyPlan,
    pub play: &'a BodyPlayIdentity,
    pub host: &'a c::HostAdvertisement,
    pub partition: &'a KernelIdentityMap,
    pub node: NodeId,
    pub call: HostCallId,
    pub contract: &'a c::HostCallContractId,
}
struct Pending {
    contribution: CommittedFaceContribution,
    committed: bool,
}
/// Exactly one retained fragment; no queue, unbounded history or Face injection.
/// Root must fund these prepared Arc/identity payloads AND the separately
/// bounded Face-construction buffers/one retained fragment before Play.
pub(crate) struct PendingFaceContributionRoute {
    state: Arc<CommitState>,
    scope: Arc<FaceContributionScope>,
    pending: Option<Pending>,
    last_request: Option<RequestId>,
}
impl PendingFaceContributionRoute {
    pub(crate) fn prepare(
        context: FaceContributionContext<'_>,
        observation: FaceCommitObservation,
    ) -> Result<Self, FaceContributionRefusal> {
        let fail = FaceContributionRefusal::Scope;
        context.plan.validate_for(context.wake).map_err(|_| fail)?;
        if !context.play.validate_for(context.plan)
            || observation.0.call != context.call
            || context.partition.placement_for_node(context.node) != Some(&observation.0.placement)
            || !context
                .partition
                .host_calls
                .iter()
                .any(|(node, call, contract)| {
                    *node == context.node && *call == context.call && contract == context.contract
                })
        {
            return Err(fail);
        }
        let mut origins = context
            .plan
            .plots
            .iter()
            .filter(|p| p.plan.plan_id == context.partition.plan_id);
        let origin = origins.next().ok_or(fail)?;
        if origins.next().is_some() {
            return Err(fail);
        }
        let fragment = origin
            .plan
            .fragments
            .iter()
            .find(|f| f.fragment_id == context.partition.fragment_id)
            .ok_or(fail)?;
        let placement = fragment
            .placements
            .iter()
            .find(|p| p.placement_id == observation.0.placement)
            .ok_or(fail)?;
        let required = placement
            .host_calls
            .get(usize::from(context.call.0))
            .filter(|call| &call.contract_id == context.contract)
            .ok_or(fail)?;
        if fragment.host_id != context.host.host_id
            || fragment.boot_id != context.host.boot_id
            || fragment.offer_generation != context.host.offer_generation
            || !fragment.placements.iter().any(|p| {
                p.placement_id == observation.0.placement
                    && p.host_id == context.host.host_id
                    && p.boot_id == context.host.boot_id
            })
            || observation.0.revoked.load(Ordering::Acquire)
            || observation.0.committed.load(Ordering::Acquire) != 0
            || observation
                .0
                .attached
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err(fail);
        }
        let scope = FaceContributionScope {
            body_play: context.play.clone(),
            host: context.host.host_id.clone(),
            boot: context.host.boot_id.clone(),
            plot: c::PlotIdentity {
                source_document_id: origin.plot.source_document_id.clone(),
                checked_plot_id: origin.plot.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            partition_plan: context.partition.plan_id.clone(),
            fragment: context.partition.fragment_id.clone(),
            placement: observation.0.placement.clone(),
            node: context.node,
            call: context.call,
            maximum_input_bytes: required.maximum_input_bytes,
            lifetime: observation.0.clone(),
            _memory: None,
        };
        Ok(Self {
            state: observation.0,
            scope: Arc::new(scope),
            pending: None,
            last_request: None,
        })
    }
    pub(crate) fn is_retired(&self) -> bool {
        self.scope.is_retired()
    }
    pub(crate) fn escaped_gate_backing_bytes(&self) -> Option<usize> {
        core::mem::size_of::<CommitState>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(self.state.placement.0.capacity())
    }
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn attach_image_memory(
        &mut self,
        lease: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        assert!(
            self.pending.is_none(),
            "IMAGE charge attaches before Face execution"
        );
        Arc::get_mut(&mut self.scope)
            .expect("unstarted private Face scope")
            ._memory = Some(lease.clone());
    }
    pub(crate) fn basis(&self) -> FaceContributionBasis {
        FaceContributionBasis {
            checked_plot_id: self.scope.plot.checked_plot_id.clone(),
            plan_id: self.scope.body_play.plan_id.clone(),
            active_play_id: self.scope.body_play.active_play_id.clone(),
            required_interaction_context: None,
        }
    }
    /// The sealed adapter accepts ONLY the actual production kernel type.
    // Preserve the caller's original fragment on pressure without allocation.
    #[allow(clippy::result_large_err)]
    pub(crate) fn stage<K: FaceContributionKernel>(
        &mut self,
        kernel: &K,
        request: HostCallRequest,
        fragment: FaceFragment,
    ) -> Result<(), (FaceFragment, FaceContributionRefusal)> {
        let refusal = if self.state.revoked.load(Ordering::Acquire) {
            Some(FaceContributionRefusal::Revoked)
        } else if self.pending.is_some() {
            Some(FaceContributionRefusal::Pressure)
        } else if request.node != self.scope.node
            || request.call != self.scope.call
            || request.input.admitted_bytes != self.scope.maximum_input_bytes
            || request.input.value.byte_len > self.scope.maximum_input_bytes
            || self
                .last_request
                .is_some_and(|last| request.request <= last)
            || !kernel.pending_face_request(request)
            || fragment.basis.checked_plot_id != self.scope.plot.checked_plot_id
            || fragment.basis.plan_id != self.scope.body_play.plan_id
            || fragment.basis.active_play_id != self.scope.body_play.active_play_id
            || fragment.basis.required_interaction_context.is_some()
            || fragment.validate_bounds().is_err()
        {
            Some(FaceContributionRefusal::Scope)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            return Err((fragment, refusal));
        }
        self.last_request = Some(request.request);
        self.pending = Some(Pending {
            contribution: CommittedFaceContribution {
                scope: self.scope.clone(),
                request: request.request,
                fragment,
            },
            committed: false,
        });
        Ok(())
    }
    /// Call after successful scheduler.step. Host completion alone cannot mint
    /// the private gate's marker. Pressure retains the exact immutable payload.
    pub(crate) fn publish(
        &mut self,
        observer: &mut dyn BodyFaceContributionObserver,
    ) -> Result<bool, FaceContributionRefusal> {
        if self.state.revoked.load(Ordering::Acquire) {
            return Err(FaceContributionRefusal::Revoked);
        }
        let Some(pending) = self.pending.as_mut() else {
            return Ok(false);
        };
        if !pending.committed
            && self.state.committed.load(Ordering::Acquire)
                != u64::from(pending.contribution.request.0) + 1
        {
            return Ok(false);
        }
        pending.committed = true;
        let pending = self
            .pending
            .take()
            .expect("observed one pending contribution");
        match observer.contribute(pending.contribution) {
            Ok(()) => Ok(true),
            Err(returned) => {
                if !Arc::ptr_eq(&returned.contribution.scope, &self.scope)
                    || Some(returned.contribution.request) != self.last_request
                {
                    return Err(FaceContributionRefusal::Scope);
                }
                self.pending = Some(Pending {
                    contribution: returned.contribution,
                    committed: true,
                });
                Err(returned.refusal)
            }
        }
    }
    /// Additional route preparation heap payload, including its Arc header and
    /// spare String capacity. Gate backing is charged separately once; inline
    /// gate/route state and the one pending Face remain separate charges.
    pub(crate) fn additional_backing_bytes(&self) -> Option<usize> {
        let s = &self.scope;
        let mut bytes = core::mem::size_of::<FaceContributionScope>()
            .checked_add(2 * core::mem::size_of::<usize>())?
            .checked_add(s.body_play.body_id.retained_backing_bytes())?
            .checked_add(s.body_play.wake_id.retained_backing_bytes())?;
        for text in [
            &s.body_play.active_play_id.0,
            &s.body_play.plan_id.0,
            &s.host.0,
            &s.boot.0,
            &s.plot.source_document_id.0,
            &s.plot.checked_plot_id.0,
            &s.plot.expanded_plot_id.0,
            &s.partition_plan.0,
            &s.fragment.0,
            &s.placement.0,
        ] {
            bytes = bytes.checked_add(text.capacity())?;
        }
        Some(bytes)
    }
}
mod sealed {
    pub trait Kernel {}
}
pub(crate) trait FaceContributionKernel: sealed::Kernel {
    fn pending_face_request(&self, request: HostCallRequest) -> bool;
}
macro_rules! kernel_impl {
    () => {
        impl<
                D: StepBack<P>,
                S: ValueStorage,
                E: SignSink,
                const N: usize,
                const C: usize,
                const P: usize,
                const Q: usize,
                const R: usize,
                const T: usize,
                const H: usize,
                const A: usize,
            > sealed::Kernel for FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>
        {
        }
        impl<
                D: StepBack<P>,
                S: ValueStorage,
                E: SignSink,
                const N: usize,
                const C: usize,
                const P: usize,
                const Q: usize,
                const R: usize,
                const T: usize,
                const H: usize,
                const A: usize,
            > FaceContributionKernel for FixedScheduler<D, S, E, N, C, P, Q, R, T, H, A>
        {
            fn pending_face_request(&self, request: HostCallRequest) -> bool {
                self.host_call_is_pending(request)
            }
        }
    };
}
kernel_impl!();
#[cfg(test)]
#[path = "face_contribution/tests.rs"]
mod tests;
