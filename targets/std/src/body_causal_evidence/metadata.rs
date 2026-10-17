//! Borrowed inspection facts for exact retained causal nodes.

use super::BodyRunCausalRecord;
use super::EventTimeCapture;
use conduit_core::{ClockProvenance, TemporalScale};
use conduit_kernel::causal_evidence::{
    BodyTimeMetadata, ClockCapture, ClockScale, ClockSourceMetadata, EvidenceIdentity,
    EvidenceMetadataFact, EvidenceMetadataLookup, EvidenceMetadataVisit, EvidenceOutcome,
};

impl EvidenceMetadataLookup for BodyRunCausalRecord {
    fn visit<'a>(
        &'a self,
        evidence: EvidenceIdentity,
        visitor: &mut dyn FnMut(EvidenceMetadataFact<'a>) -> bool,
    ) -> EvidenceMetadataVisit {
        let Some(node) = self.nodes.iter().find(|node| node.evidence == evidence) else {
            return EvidenceMetadataVisit::Missing;
        };
        let outcome = if node.semantic_terminal {
            EvidenceOutcome::SemanticTerminal
        } else {
            node.outcome
        };
        let mut emit = |fact| {
            if visitor(fact) {
                Ok(())
            } else {
                Err(EvidenceMetadataVisit::VisitorRefused)
            }
        };
        let result = (|| {
            emit(EvidenceMetadataFact::Outcome(outcome))?;
            emit(EvidenceMetadataFact::SemanticSubject {
                gear: node.gear_id.as_str(),
                kind: node.kind_id.as_str(),
            })?;
            emit(EvidenceMetadataFact::Source {
                document: node.source_document_id.as_str(),
                start: node.source_span.map(|span| span.start),
                end: node.source_span.map(|span| span.end),
                line: node.source_span.map(|span| span.line),
                column: node.source_span.map(|span| span.column),
                end_line: node.source_span.map(|span| span.end_line),
                end_column: node.source_span.map(|span| span.end_column),
            })?;
            emit(EvidenceMetadataFact::Wake(node.wake_id.as_str()))?;
            emit(EvidenceMetadataFact::Plan(node.plan_id.as_str()))?;
            emit(EvidenceMetadataFact::Play(node.play_id.as_str()))?;
            emit(EvidenceMetadataFact::Placement(node.placement_id.as_str()))?;
            emit(EvidenceMetadataFact::Implementation(
                node.implementation_id.as_str(),
            ))?;
            emit(EvidenceMetadataFact::Host(node.host_id.as_str()))?;
            emit(EvidenceMetadataFact::Boot(node.boot_id.as_str()))?;
            emit(EvidenceMetadataFact::LocalOrder {
                sequence: node.kernel_sequence,
            })?;
            if let Some(observation) = &node.observed_time {
                let local = observation.local();
                let body = observation.body().map(|estimate| BodyTimeMetadata {
                    basis: &estimate.body_basis,
                    generation: estimate.generation,
                    correlation_age_ticks: estimate.correlation_age_ticks,
                    correlation_age_scale: clock_scale(estimate.local_sample.clock().scale()),
                    earliest_ticks: estimate.earliest_ticks,
                    center_ticks: estimate.center_ticks,
                    latest_ticks: estimate.latest_ticks,
                    scale: clock_scale(estimate.scale),
                    source: match &estimate.provenance {
                        ClockProvenance::Peer {
                            host_id,
                            boot_id,
                            policy_id,
                            ..
                        } => ClockSourceMetadata::Peer {
                            host: host_id.as_str(),
                            boot: boot_id.as_str(),
                            policy: policy_id,
                        },
                        ClockProvenance::External {
                            provider_id,
                            policy_id,
                            ..
                        } => ClockSourceMetadata::External {
                            provider: provider_id,
                            policy: policy_id,
                        },
                    },
                });
                emit(EvidenceMetadataFact::ClockObservation {
                    capture: match observation.capture() {
                        EventTimeCapture::AtEvent => ClockCapture::AtEvent,
                        EventTimeCapture::AfterEvent => ClockCapture::AfterEvent,
                    },
                    local_ticks: local.ticks(),
                    local_scale: clock_scale(local.clock().scale()),
                    local_basis: local.clock().basis_id(),
                    body,
                })?;
            }
            for resource in &node.resources {
                emit(EvidenceMetadataFact::Resource {
                    pool: resource.pool_id.as_str(),
                    generation: None,
                })?;
            }
            for authority in &node.authority {
                emit(EvidenceMetadataFact::Authority {
                    grant: authority.grant_id.as_str(),
                    contract: authority.contract_id.as_str(),
                })?;
            }
            Ok(())
        })();
        match result {
            Ok(()) => EvidenceMetadataVisit::Visited,
            Err(refusal) => refusal,
        }
    }
}

const fn clock_scale(scale: TemporalScale) -> ClockScale {
    match scale {
        TemporalScale::Seconds => ClockScale::Seconds,
        TemporalScale::Milliseconds => ClockScale::Milliseconds,
        TemporalScale::Microseconds => ClockScale::Microseconds,
        TemporalScale::Nanoseconds => ClockScale::Nanoseconds,
    }
}
