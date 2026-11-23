//! Home projection over the existing original IMAGE and committed Face route.
use super::InstalledScheduler;
use crate::body_execution::{
    face_contribution::BodyFaceContributionObserver,
    face_contribution::{
        FaceCommitObservation, FaceContributionContext, FaceContributionRefusal,
        PendingFaceContributionRoute,
    },
};
use conduit_kernel::{
    scheduler::HostCallRequest, HostCallDisposition, HostCallId, HostCallOutcome, NodeId,
};

// One accepted current frame and one replacement. A receiver retaining both
// applies pressure until it releases one; total updates do not enlarge storage.
const RETAINED_HOME_FRAGMENTS: usize = 2;

pub(in crate::installed_std) struct AmbientHomeRoute {
    node: NodeId,
    label: String,
    route: PendingFaceContributionRoute,
    faces: conduit_semantic_catalog::PreparedAmbientFaces,
    built: Option<HostCallRequest>,
    custody: Option<HostCallRequest>,
    waiting_projection: Option<HostCallRequest>,
    memory: Option<crate::kernel_preparation::ImageMemoryLease>,
}

impl AmbientHomeRoute {
    pub(super) fn preparation_backing_bound(
        body: &conduit_body::BodyPlan,
        plot: &conduit_body::BodyPlotPlan,
        fragment: &conduit_core::PlanFragment,
        placement: &conduit_core::PlannedGear,
    ) -> Result<usize, String> {
        let label = super::ambient_home_factory::label(placement)?;
        let workspace = conduit_semantic_catalog::ambient_home_workspace_bytes()? as usize;
        let (scope, gate) = PendingFaceContributionRoute::preparation_backing_bound(
            body, plot, fragment, placement,
        )
        .ok_or("Home Face scope backing overflow")?;
        core::mem::size_of::<Self>()
            .checked_add(workspace)
            .and_then(|n| n.checked_add(label.len()))
            .and_then(|n| n.checked_add(scope.checked_mul(2)?))
            .and_then(|n| n.checked_add(gate.checked_mul(2)?))
            .and_then(|n| n.checked_add(escaped_fragment_bytes()?))
            .ok_or_else(|| "Home Face preparation backing overflow".into())
    }

    /// The caller pays model preparation, decode/projection workspace and the
    /// finite escaped fragment envelope before starting the original IMAGE.
    /// Preparation precedes the actual IMAGE receipt. Dispatch remains fenced
    /// until the original admitted lease is attached after that receipt.
    pub(in crate::installed_std) fn prepare(
        context: FaceContributionContext<'_>,
        observation: FaceCommitObservation,
        label: &str,
    ) -> Result<Self, String> {
        use conduit_std_offers::{AMBIENT_HOME_CUSTODY_CALL, AMBIENT_HOME_PROJECT_CALL};
        if !conduit_core::valid_data_label(label)
            || context.call != HostCallId(0)
            || context.contract.as_str() != AMBIENT_HOME_PROJECT_CALL
            || !context
                .partition
                .host_calls
                .iter()
                .any(|(node, call, contract)| {
                    *node == context.node
                        && *call == HostCallId(1)
                        && contract.as_str() == AMBIENT_HOME_CUSTODY_CALL
                })
        {
            return Err("Home requires its two exact planned operations".into());
        }
        let node = context.node;
        let faces = conduit_semantic_catalog::PreparedAmbientFaces::prepare()
            .map_err(|why| format!("Home model preparation: {why:?}"))?;
        let route = PendingFaceContributionRoute::prepare(context, observation)
            .map_err(|why| format!("Home contribution scope: {why:?}"))?;
        Ok(Self {
            node,
            label: label.to_owned(),
            route,
            faces,
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
            .checked_add(self.faces.retained_backing_bytes()?)?
            .checked_add(self.label.capacity())
    }

    pub(in crate::installed_std) fn additional_backing_bytes(&self) -> Result<usize, String> {
        self.prepared_heap_backing_bytes()
            .and_then(|n| n.checked_add(self.faces.projection_workspace_bytes()?))
            .ok_or_else(|| "Home prepared/projection backing overflow".into())
    }
    pub(in crate::installed_std) fn escaped_backing_bytes(&self) -> Result<usize, String> {
        escaped_fragment_bytes()
            .and_then(|n| n.checked_add(self.route.additional_backing_bytes()?))
            .and_then(|n| n.checked_add(self.route.escaped_gate_backing_bytes()?))
            .ok_or_else(|| "Home finite escaped backing overflow".into())
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
            return Err("Home request lost its exact kernel scope".into());
        }
        match request.call {
            HostCallId(0) => {
                if self.built.is_some() || self.route.is_pending() {
                    return Err("Home still retains its earlier contribution".into());
                }
                if self
                    .waiting_projection
                    .is_some_and(|pending| pending != request)
                {
                    return Err("Home retained another pending projection".into());
                }
                if self.route.retained_fragments() >= RETAINED_HOME_FRAGMENTS {
                    self.waiting_projection = Some(request);
                    return Ok(());
                }
                self.waiting_projection = None;
                let bytes = kernel
                    .host_value(request.input.value)
                    .map_err(|why| format!("Home original input: {why:?}"))?;
                let mut fragment = self
                    .faces
                    .home_canonical_observation(self.route.basis(), bytes)
                    .map_err(|why| format!("Home semantic projection: {why:?}"))?;
                fragment
                    .subjects
                    .first_mut()
                    .ok_or("Home projection has no root")?
                    .name = self.label.clone();
                let backing = conduit_semantic_catalog::ambient_fragment_backing_bytes(&fragment)
                    .ok_or("Home fragment has unsupported backing")?;
                if backing > conduit_semantic_catalog::ambient_home_fragment_backing_bound() {
                    return Err("Home fragment exceeds its admitted envelope".into());
                }
                self.route
                    .stage(kernel, request, fragment)
                    .map_err(|(_, why)| format!("Home contribution staging: {why:?}"))?;
                complete(kernel, request)?;
                self.built = Some(request);
            }
            HostCallId(1) => {
                let built = self
                    .built
                    .ok_or("Home custody has no original projection")?;
                if built.request.0.checked_add(1) != Some(request.request.0)
                    || built.input != request.input
                    || !self.route.is_pending()
                {
                    return Err("Home custody differs from its original projection".into());
                }
                self.custody = Some(request);
            }
            _ => return Err("Home operation is not planned by this route".into()),
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
            return Err("Home contribution retired".into());
        }
        if let Some(request) = self.waiting_projection {
            if !kernel.host_call_is_pending(request) {
                return Err("Home retained projection lost its pending request".into());
            }
            if self.route.retained_fragments() >= RETAINED_HOME_FRAGMENTS {
                return Ok(false);
            }
            self.dispatch(kernel, request)?;
            return Ok(true);
        }
        let Some(request) = self.custody else {
            return Ok(false);
        };
        if !kernel.host_call_is_pending(request) {
            return Err("Home observer custody lost its pending request".into());
        }
        match self.route.publish(observer) {
            Ok(true) => {
                complete(kernel, request)?;
                self.custody = None;
                self.built = None;
                Ok(true)
            }
            Ok(false) | Err(FaceContributionRefusal::Pressure) => Ok(false),
            Err(why) => Err(format!("Home observer refused contribution: {why:?}")),
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

fn complete(kernel: &mut InstalledScheduler, request: HostCallRequest) -> Result<(), String> {
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
        .map_err(|why| format!("Home Host Call completion: {why:?}"))
}

fn escaped_fragment_bytes() -> Option<usize> {
    conduit_semantic_catalog::ambient_home_fragment_backing_bound()
        .checked_add(core::mem::size_of::<
            crate::body_execution::CommittedFaceContribution,
        >())?
        .checked_mul(RETAINED_HOME_FRAGMENTS)
}
