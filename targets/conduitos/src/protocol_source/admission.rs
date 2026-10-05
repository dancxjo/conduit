//! Keep packaged Source and retained operation owners together through admission.
use super::*;
use crate::protocol_artifact::{AdmittedProtocolArtifact, ProtocolArtifactIdentity};
use conduit_core::{ArtifactId, BaseImplementationId, HostAdvertisement};
use conduit_planner::{PlacementChoices, PlanningOptions};
use conduit_plot::ExpandedAuthoringPlot;

pub struct PreparedProtocolArtifact {
    artifact: AdmittedProtocolArtifact,
    operations: ProtocolOperations,
}

impl PreparedProtocolSource {
    /// Native callers supply selected offers and grants. This entrance neither
    /// discovers providers nor creates authority from package metadata.
    pub fn plan_artifact(
        self,
        expanded: &ExpandedAuthoringPlot,
        artifact: ArtifactId,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
        bases: &[BaseImplementationId],
        options: PlanningOptions<'_>,
    ) -> Result<PreparedProtocolArtifact, ProtocolSourceRefusal> {
        let limits = self.queue_limits(expanded, hosts, placements)?;
        self.plan_with_limits(
            expanded, artifact, hosts, placements, bases, options, limits,
        )
    }

    pub(super) fn plan_retained_artifact(
        self,
        expanded: &ExpandedAuthoringPlot,
        artifact: ArtifactId,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
        bases: &[BaseImplementationId],
        options: PlanningOptions<'_>,
    ) -> Result<PreparedProtocolArtifact, ProtocolSourceRefusal> {
        let limits = self.retained_queue_limits(expanded, hosts, placements)?;
        self.plan_with_limits(
            expanded, artifact, hosts, placements, bases, options, limits,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn plan_with_limits(
        self,
        expanded: &ExpandedAuthoringPlot,
        artifact: ArtifactId,
        hosts: &[HostAdvertisement],
        placements: &PlacementChoices,
        bases: &[BaseImplementationId],
        options: PlanningOptions<'_>,
        limits: ProtocolQueueLimits,
    ) -> Result<PreparedProtocolArtifact, ProtocolSourceRefusal> {
        let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
            expanded,
            hosts,
            placements,
            bases,
            options,
            &limits.connections,
            &limits.boundaries,
        )
        .map_err(ProtocolSourceRefusal::Plan)?;
        self.operations
            .states
            .validate_plan(&plan)
            .map_err(|_| ProtocolSourceRefusal::Offer)?;
        self.operations
            .joins
            .validate_plan(&plan)
            .map_err(|_| ProtocolSourceRefusal::Offer)?;
        self.operations
            .merges
            .validate_plan(&plan)
            .map_err(|_| ProtocolSourceRefusal::Offer)?;
        let identity = ProtocolArtifactIdentity {
            source: self.checked.source_document_id.clone(),
            checked: expanded.expanded.checked_plot_id.clone(),
            expanded: expanded.expanded.expanded_plot_id.clone(),
            artifact,
        };
        let artifact = AdmittedProtocolArtifact::admit(identity, plan)
            .map_err(ProtocolSourceRefusal::Admission)?;
        Ok(PreparedProtocolArtifact {
            artifact,
            operations: self.operations,
        })
    }
}

impl PreparedProtocolArtifact {
    pub fn artifact(&self) -> &AdmittedProtocolArtifact {
        &self.artifact
    }

    /// Contribute the exact retained protocol Plan to ordinary body admission.
    /// The caller seals every current resident partition together with
    /// `BodyPlan::seal`; this projection creates neither a Wake nor a Play and
    /// leaves the operation owners retained by this artifact.
    pub fn body_partition(&self) -> conduit_body::BodyPlotPlan {
        let identity = self.artifact.identity();
        conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                identity.source.clone(),
                identity.checked.clone(),
            ),
            plan: self.artifact.definition().internal_plan.clone(),
        }
    }

    /// Consume the retained package owners together with actual native possession.
    pub fn prepare_timed<P, C>(
        self,
        bus: crate::protocol_play::I2cAdmission<P>,
        clock: crate::protocol_play::ClockAdmission<C>,
    ) -> Result<
        crate::protocol_play::PreparedTimedProtocolPlay<P, C>,
        crate::protocol_host_calls::ProtocolCallRefusal,
    >
    where
        P: crate::i2c_base::I2cProvider,
        C: crate::monotonic_clock::owner::MonotonicDeadlineProvider,
    {
        crate::protocol_play::PreparedTimedProtocolPlay::prepare(
            self.artifact.into_definition(),
            bus.ready,
            bus.table,
            bus.handle,
            bus.claim,
            self.operations,
            clock,
        )
    }
}
