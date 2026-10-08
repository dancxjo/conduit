//! Optional event-time facts remain separate from the exact causal graph.

use conduit_core::{BodyTimeEstimate, BodyTimeRelation, MonotonicInstant};
use serde::{Deserialize, Serialize};

use super::{BodyCausalEvidenceRefusal, BodyRunCausalRecord, EvidenceIdentity};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyEventTimeObservation {
    local: MonotonicInstant,
    body: Option<BodyTimeEstimate>,
}

impl BodyEventTimeObservation {
    pub fn new(
        local: MonotonicInstant,
        body: Option<BodyTimeEstimate>,
    ) -> Result<Self, BodyCausalEvidenceRefusal> {
        let value = Self { local, body };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), BodyCausalEvidenceRefusal> {
        if self.local.validate().is_err()
            || self
                .body
                .as_ref()
                .is_some_and(|body| body.validate().is_err() || body.local_sample != self.local)
        {
            return Err(BodyCausalEvidenceRefusal::InvalidClockObservation);
        }
        Ok(())
    }

    pub const fn local(&self) -> &MonotonicInstant {
        &self.local
    }

    pub const fn body(&self) -> Option<&BodyTimeEstimate> {
        self.body.as_ref()
    }
}

impl BodyRunCausalRecord {
    pub fn attach_time(
        &mut self,
        evidence: EvidenceIdentity,
        observation: BodyEventTimeObservation,
    ) -> Result<(), BodyCausalEvidenceRefusal> {
        observation.validate()?;
        let node = self
            .nodes
            .iter_mut()
            .find(|node| node.evidence == evidence)
            .ok_or(BodyCausalEvidenceRefusal::UnknownClockEvent)?;
        if observation.local.clock().host_id() != &node.host_id
            || observation.local.clock().boot_id() != &node.boot_id
            || observation
                .body()
                .is_some_and(|body| body.body_basis != node.body_id.as_str())
        {
            return Err(BodyCausalEvidenceRefusal::InvalidClockObservation);
        }
        if node.observed_time.is_some() {
            return Err(BodyCausalEvidenceRefusal::ConflictingClockObservation);
        }
        node.observed_time = Some(observation);
        Ok(())
    }

    pub fn time_of(&self, evidence: EvidenceIdentity) -> Option<&BodyEventTimeObservation> {
        self.nodes
            .iter()
            .find(|node| node.evidence == evidence)
            .and_then(|node| node.observed_time.as_ref())
    }

    pub fn physical_relation(
        &self,
        first: EvidenceIdentity,
        second: EvidenceIdentity,
    ) -> Result<Option<BodyTimeRelation>, BodyCausalEvidenceRefusal> {
        let Some(first) = self.time_of(first).and_then(BodyEventTimeObservation::body) else {
            return Ok(None);
        };
        let Some(second) = self
            .time_of(second)
            .and_then(BodyEventTimeObservation::body)
        else {
            return Ok(None);
        };
        first
            .physical_relation(second)
            .map(Some)
            .map_err(|_| BodyCausalEvidenceRefusal::InvalidClockObservation)
    }
}
