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
