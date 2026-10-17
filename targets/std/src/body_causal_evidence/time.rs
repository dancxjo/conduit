//! Optional event-time facts remain separate from the exact causal graph.

use conduit_core::{BodyClockCorrelation, BodyTimeEstimate, BodyTimeRelation, MonotonicInstant};
use serde::{Deserialize, Serialize};

use super::{BodyCausalEvidenceRefusal, BodyRunCausalRecord, EvidenceIdentity};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventTimeCapture {
    AtEvent,
    AfterEvent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyEventTimeObservation {
    local: MonotonicInstant,
    body: Option<BodyTimeEstimate>,
    correlation: Option<BodyClockCorrelation>,
    capture: EventTimeCapture,
}

impl BodyEventTimeObservation {
    pub fn new(
        local: MonotonicInstant,
        correlation: Option<BodyClockCorrelation>,
    ) -> Result<Self, BodyCausalEvidenceRefusal> {
        let body = correlation
            .as_ref()
            .map(|correlation| correlation.project(&local))
            .transpose()
            .map_err(|_| BodyCausalEvidenceRefusal::InvalidClockObservation)?;
        let value = Self {
            local,
            body,
            correlation,
            capture: EventTimeCapture::AtEvent,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn captured_after(local: MonotonicInstant) -> Result<Self, BodyCausalEvidenceRefusal> {
        let value = Self {
            local,
            body: None,
            correlation: None,
            capture: EventTimeCapture::AfterEvent,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn validate(&self) -> Result<(), BodyCausalEvidenceRefusal> {
        if self.local.validate().is_err()
            || match (&self.correlation, &self.body) {
                (Some(correlation), Some(body)) => {
                    correlation.project(&self.local).ok().as_ref() != Some(body)
                }
                (None, None) => false,
                _ => true,
            }
            || (self.capture == EventTimeCapture::AfterEvent && self.correlation.is_some())
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

    pub const fn correlation(&self) -> Option<&BodyClockCorrelation> {
        self.correlation.as_ref()
    }

    pub const fn capture(&self) -> EventTimeCapture {
        self.capture
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
