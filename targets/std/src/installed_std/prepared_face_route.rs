//! Shared original-IMAGE dispatch and observer custody for prepared Face projectors.
use super::InstalledScheduler;
use crate::body_execution::{
    face_contribution::BodyFaceContributionObserver,
    face_contribution::{
        FaceCommitObservation, FaceContributionContext, FaceContributionRefusal,
        PendingFaceContributionRoute,
    },
};
use conduit_face::{FaceContributionBasis, FaceFragment};
use conduit_kernel::{
    scheduler::HostCallRequest, HostCallDisposition, HostCallId, HostCallOutcome, NodeId,
};

/// Prepared semantic construction only: no dispatch, publication, or IMAGE authority.
/// Heap visitors exclude inline projector state, which the route charges separately.
pub(super) trait PreparedFaceProjector {
    const NAME: &'static str;
    fn project(
        &self,
        basis: FaceContributionBasis,
        bytes: &[u8],
        label: &str,
    ) -> Result<FaceFragment, String>;
    fn retained_backing_bytes(&self) -> Option<usize>;
    fn projection_workspace_bytes(&self) -> Option<usize>;
    fn fragment_backing_bound(&self) -> usize;
    fn fragment_backing_bytes(&self, fragment: &FaceFragment) -> Option<usize>;
}

// One accepted current frame and one replacement. A receiver retaining both
// applies pressure until it releases one; total updates do not enlarge storage.
const RETAINED_FRAGMENTS: usize = 2;

pub(in crate::installed_std) struct PreparedFaceRoute<P: PreparedFaceProjector> {
    node: NodeId,
    projector: P,
    label: String,
    route: PendingFaceContributionRoute,
    built: Option<HostCallRequest>,
    custody: Option<HostCallRequest>,
    waiting_projection: Option<HostCallRequest>,
    memory: Option<crate::kernel_preparation::ImageMemoryLease>,
}

impl<P: PreparedFaceProjector> PreparedFaceRoute<P> {
    /// `workspace` covers retained projector heap plus transient decode/projection work.
    pub(super) fn preparation_backing_bound(
        body: &conduit_body::BodyPlan,
        plot: &conduit_body::BodyPlotPlan,
        fragment: &conduit_core::PlanFragment,
        placement: &conduit_core::PlannedGear,
        label_len: usize,
        workspace: usize,
        fragment_bound: usize,
    ) -> Result<usize, String> {
        let (scope, gate) = PendingFaceContributionRoute::preparation_backing_bound(
            body, plot, fragment, placement,
        )
        .ok_or_else(|| format!("{} Face scope backing overflow", P::NAME))?;
        core::mem::size_of::<Self>()
            .checked_add(workspace)
            .and_then(|n| n.checked_add(label_len))
            .and_then(|n| n.checked_add(scope.checked_mul(2)?))
            .and_then(|n| n.checked_add(gate.checked_mul(2)?))
            .and_then(|n| n.checked_add(escaped_fragment_bytes(fragment_bound)?))
            .ok_or_else(|| format!("{} Face preparation backing overflow", P::NAME))
    }

    /// The caller pays model preparation, decode/projection workspace and the
    /// finite escaped fragment envelope before starting the original IMAGE.
    /// Preparation precedes the actual IMAGE receipt. Dispatch remains fenced
    /// until the original admitted lease is attached after that receipt.
    pub(in crate::installed_std) fn prepare(
        context: FaceContributionContext<'_>,
        observation: FaceCommitObservation,
        label: &str,
        calls: (&str, &str),
        prepare_projector: impl FnOnce() -> Result<P, String>,
    ) -> Result<Self, String> {
        let (project, custody) = calls;
        if !conduit_core::valid_data_label(label)
            || context.call != HostCallId(0)
            || context.contract.as_str() != project
            || !context
                .partition
                .host_calls
                .iter()
                .any(|(node, call, contract)| {
                    *node == context.node && *call == HostCallId(1) && contract.as_str() == custody
                })
        {
            return Err(format!(
                "{} requires its two exact planned operations",
                P::NAME
            ));
        }
        let node = context.node;
        let projector = prepare_projector()?;
        let route = PendingFaceContributionRoute::prepare(context, observation)
            .map_err(|why| format!("{} contribution scope: {why:?}", P::NAME))?;
        Ok(Self {
            node,
            projector,
            label: label.to_owned(),
            route,
            built: None,
            custody: None,
            waiting_projection: None,
            memory: None,
        })
    }

    pub(in crate::installed_std) fn attach_image_memory(
        &mut self,
        memory: &crate::kernel_preparation::ImageMemoryLease,
    ) {
        assert!(self.memory.is_none() && self.built.is_none());
        self.route.attach_image_memory(memory);
        self.memory = Some(memory.clone());
    }

    /// Prepared heap payload, excluding inline Self, transient projection work
    /// and escaped frames. Those charges must be added by the owning receipt.
    pub(in crate::installed_std) fn prepared_heap_backing_bytes(&self) -> Option<usize> {
        self.route
            .additional_backing_bytes()?
            .checked_add(self.projector.retained_backing_bytes()?)?
            .checked_add(self.label.capacity())
    }

    pub(in crate::installed_std) fn additional_backing_bytes(&self) -> Result<usize, String> {
        self.prepared_heap_backing_bytes()
            .and_then(|n| n.checked_add(self.projector.projection_workspace_bytes()?))
            .ok_or_else(|| format!("{} prepared/projection backing overflow", P::NAME))
    }
    pub(in crate::installed_std) fn escaped_backing_bytes(&self) -> Result<usize, String> {
        escaped_fragment_bytes(self.projector.fragment_backing_bound())
            .and_then(|n| n.checked_add(self.route.additional_backing_bytes()?))
            .and_then(|n| n.checked_add(self.route.escaped_gate_backing_bytes()?))
            .ok_or_else(|| format!("{} finite escaped backing overflow", P::NAME))
    }

    /// Called only for the actual dispatched request, preserving kernel input
    /// custody through projection and the later observer acceptance call.
    pub(in crate::installed_std) fn dispatch(
        &mut self,
        kernel: &mut InstalledScheduler,
        request: HostCallRequest,
    ) -> Result<(), String> {
        if self.memory.is_none()
            || request.node != self.node
            || !kernel.host_call_is_pending(request)
            || self.route.is_retired()
            || self.custody.is_some()
        {
            return Err(format!("{} request lost its exact kernel scope", P::NAME));
        }
        match request.call {
            HostCallId(0) => {
                if self.built.is_some() || self.route.is_pending() {
                    return Err(format!(
                        "{} still retains its earlier contribution",
                        P::NAME
                    ));
                }
                if self
                    .waiting_projection
                    .is_some_and(|pending| pending != request)
                {
                    return Err(format!("{} retained another pending projection", P::NAME));
                }
                if self.route.retained_fragments() >= RETAINED_FRAGMENTS {
                    self.waiting_projection = Some(request);
                    return Ok(());
                }
                self.waiting_projection = None;
                let bytes = kernel
                    .host_value(request.input.value)
                    .map_err(|why| format!("{} original input: {why:?}", P::NAME))?;
                let basis = self.route.basis();
                let fragment = self.projector.project(basis, bytes, &self.label)?;
                let backing = self
                    .projector
                    .fragment_backing_bytes(&fragment)
                    .ok_or_else(|| format!("{} fragment has unsupported backing", P::NAME))?;
                if backing > self.projector.fragment_backing_bound() {
                    return Err(format!(
                        "{} fragment exceeds its admitted envelope",
                        P::NAME
                    ));
                }
                self.route
                    .stage(kernel, request, fragment)
                    .map_err(|(_, why)| format!("{} contribution staging: {why:?}", P::NAME))?;
                complete::<P>(kernel, request)?;
                self.built = Some(request);
            }
            HostCallId(1) => {
                let built = self
                    .built
                    .ok_or_else(|| format!("{} custody has no original projection", P::NAME))?;
                if built.request.0.checked_add(1) != Some(request.request.0)
                    || built.input != request.input
                    || !self.route.is_pending()
                {
                    return Err(format!(
                        "{} custody differs from its original projection",
                        P::NAME
                    ));
                }
                self.custody = Some(request);
            }
            _ => {
                return Err(format!(
                    "{} operation is not planned by this route",
                    P::NAME
                ));
            }
        }
        Ok(())
    }

    /// Observer acceptance transfers semantic custody; it proves no physical
    /// Show, durable recording, or human acknowledgment. Pressure retains the
    /// original charged fragment and leaves the second kernel call pending.
    pub(in crate::installed_std) fn poll(
        &mut self,
        kernel: &mut InstalledScheduler,
        observer: &mut dyn BodyFaceContributionObserver,
    ) -> Result<bool, String> {
        if self.route.is_retired() {
            return Err(format!("{} contribution retired", P::NAME));
        }
        if let Some(request) = self.waiting_projection {
            if !kernel.host_call_is_pending(request) {
                return Err(format!(
                    "{} retained projection lost its pending request",
                    P::NAME
                ));
            }
            if self.route.retained_fragments() >= RETAINED_FRAGMENTS {
                return Ok(false);
            }
            self.dispatch(kernel, request)?;
            return Ok(true);
        }
        let Some(request) = self.custody else {
            return Ok(false);
        };
        if !kernel.host_call_is_pending(request) {
            return Err(format!(
                "{} observer custody lost its pending request",
                P::NAME
            ));
        }
        match self.route.publish(observer) {
            Ok(true) => {
                complete::<P>(kernel, request)?;
                self.custody = None;
                self.built = None;
                Ok(true)
            }
            Ok(false) | Err(FaceContributionRefusal::Pressure) => Ok(false),
            Err(why) => Err(format!(
                "{} observer refused contribution: {why:?}",
                P::NAME
            )),
        }
    }

    pub(in crate::installed_std) fn handles(&self, request: HostCallRequest) -> bool {
        request.node == self.node && matches!(request.call, HostCallId(0) | HostCallId(1))
    }

    pub(in crate::installed_std) fn is_pending(&self) -> bool {
        self.route.is_pending()
            || self.built.is_some()
            || self.custody.is_some()
            || self.waiting_projection.is_some()
    }

    pub(in crate::installed_std) fn is_retired(&self) -> bool {
        self.route.is_retired()
    }
}

fn complete<P: PreparedFaceProjector>(
    kernel: &mut InstalledScheduler,
    request: HostCallRequest,
) -> Result<(), String> {
    kernel
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|why| format!("{} Host Call completion: {why:?}", P::NAME))
}

fn escaped_fragment_bytes(fragment_bound: usize) -> Option<usize> {
    fragment_bound
        .checked_add(core::mem::size_of::<
            crate::body_execution::CommittedFaceContribution,
        >())?
        .checked_mul(RETAINED_FRAGMENTS)
}
