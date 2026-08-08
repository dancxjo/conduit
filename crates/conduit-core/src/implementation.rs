use serde::{Deserialize, Serialize};

use crate::{ArtifactId, ExecutionProfileId, ImplementationId};

/// One exact executable realization offered beneath a semantic capability face.
///
/// These are stable realization facts. Current availability and utilization are
/// deliberately not part of this value and belong to planner observations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationOffer {
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub artifact_id: ArtifactId,
}
