//! Read-only Plan/Play projections and fail-closed Run admission.

use conduit_core::{verify_plan, HostAdvertisement, Plan, PlanId, TerminalDisposition};
use conduit_std_host::StdRunReport;

const MAXIMUM_CONTROL_ID_BYTES: usize = 128;
const MAXIMUM_INSPECTION_LINES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayRequestId(String);

impl PatchbayRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, ControlError> {
        let value = value.into();
        if value.is_empty() || value.len() > MAXIMUM_CONTROL_ID_BYTES {
            return Err(ControlError::InvalidRequestIdentity);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlError {
    InvalidRequestIdentity,
    StalePlan,
    StaleBoot,
    UnavailableRealization,
    AuthorityDenied,
    InvalidPlan,
    InspectionTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDocument {
    pub request_id: PatchbayRequestId,
    pub plan_id: PlanId,
    pub lines: Vec<String>,
}

impl PlanDocument {
    pub fn from_plan(request_id: PatchbayRequestId, plan: &Plan) -> Result<Self, ControlError> {
        let mut lines = vec![
            format!(
                "PLAN request={} plan={}",
                request_id.as_str(),
                plan.plan_id.as_str()
            ),
            format!(
                "  source={} checked={} expanded={}",
                plan.source_document_id.as_str(),
                plan.checked_form_id.as_str(),
                plan.expanded_form_id.as_str()
            ),
        ];
        for fragment in &plan.fragments {
            push(
                &mut lines,
                format!(
                    "  FRAGMENT host={} boot={} generation={}",
                    fragment.host_id.as_str(),
                    fragment.boot_id.as_str(),
                    fragment.offer_generation.0
                ),
            )?;
            for placement in &fragment.placements {
                push(&mut lines, format!(
                    "    CELL operation={} placement={} host={} boot={} capability={} implementation={} artifact={}",
                    placement.operation_id.as_str(), placement.placement_id.as_str(),
                    placement.host_id.as_str(), placement.boot_id.as_str(),
                    placement.capability_id.as_str(), placement.implementation_id.as_str(),
                    placement.artifact_id.as_str()
                ))?;
            }
            for connection in &fragment.connections {
                push(
                    &mut lines,
                    format!(
                        "    CORD connection={} provider={:?} items={} bytes={}",
                        connection.connection_id.as_str(),
                        connection.provider,
                        connection.item_capacity,
                        connection.byte_capacity
                    ),
                )?;
                for (index, candidate) in connection.route_candidates.iter().enumerate() {
                    push(
                        &mut lines,
                        format!(
                            "      CANDIDATE index={} binding={} provider={:?} instance={}",
                            index,
                            candidate.binding_id.as_str(),
                            candidate.provider,
                            candidate.provider_instance_id.as_str()
                        ),
                    )?;
                }
            }
        }
        Ok(Self {
            request_id,
            plan_id: plan.plan_id.clone(),
            lines,
        })
    }
}

pub fn admit_run(
    plan: &Plan,
    current_source: &conduit_core::SourceDocumentId,
    realm: &[HostAdvertisement],
) -> Result<(), ControlError> {
    if &plan.source_document_id != current_source {
        return Err(ControlError::StalePlan);
    }
    for fragment in &plan.fragments {
        let host = realm
            .iter()
            .find(|host| host.host_id == fragment.host_id)
            .ok_or(ControlError::UnavailableRealization)?;
        if host.boot_id != fragment.boot_id || host.offer_generation != fragment.offer_generation {
            return Err(ControlError::StaleBoot);
        }
        for placement in &fragment.placements {
            let offer = host
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .ok_or(ControlError::UnavailableRealization)?;
            if offer.implementation.implementation_id != placement.implementation_id {
                return Err(ControlError::UnavailableRealization);
            }
            if placement.authority.len() != offer.authority_requirements.len() {
                return Err(ControlError::AuthorityDenied);
            }
        }
    }
    if !verify_plan(plan) {
        return Err(ControlError::InvalidPlan);
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayDocument {
    pub lines: Vec<String>,
    pub terminal: TerminalDisposition,
}

impl PlayDocument {
    pub fn from_report(plan: &Plan, report: &StdRunReport) -> Result<Self, ControlError> {
        let kernel = report.kernel.as_ref().ok_or(ControlError::InvalidPlan)?;
        let terminal = report
            .observations
            .iter()
            .rev()
            .find_map(|observation| match observation.kind {
                conduit_core::ObservationKind::PlanTerminal { disposition } => Some(disposition),
                _ => None,
            })
            .ok_or(ControlError::InvalidPlan)?;
        let mut lines = vec![format!(
            "PLAY active={} plan={} terminal={terminal:?}",
            kernel.active_play_id.as_str(),
            plan.plan_id.as_str()
        )];
        push(
            &mut lines,
            format!(
                "  PRESSURE exposed=false decisions={} kernel_events={} evidence_gaps=0",
                kernel.decisions, kernel.kernel_events
            ),
        )?;
        for observation in &report.observations {
            push(
                &mut lines,
                format!(
                    "  EVIDENCE id={} kind={:?}",
                    observation.evidence_id.as_str(),
                    observation.kind
                ),
            )?;
        }
        for event in &kernel.kernel_evidence {
            push(
                &mut lines,
                format!(
                    "  KERNEL-EVIDENCE sequence={} node={} kind={:?}",
                    event.sequence, event.node.0, event.kind
                ),
            )?;
        }
        for receipt in &report.control_receipts {
            push(
                &mut lines,
                format!(
                    "  CONTROL request={} disposition={:?} active={}",
                    receipt.request_id.as_str(),
                    receipt.disposition,
                    receipt.active_play_id.as_str()
                ),
            )?;
        }
        Ok(Self { lines, terminal })
    }
}

fn push(lines: &mut Vec<String>, line: String) -> Result<(), ControlError> {
    if lines.len() == MAXIMUM_INSPECTION_LINES {
        return Err(ControlError::InspectionTooLarge);
    }
    lines.push(line);
    Ok(())
}

#[cfg(test)]
#[path = "control_tests.rs"]
mod tests;
