//! Atomic adoption of one exact post-birth membership biography extension.

use super::{navigation_state, PatchbayHtmlServer, ServerError};
use conduit_body::{BodyBiographyEvidence, BodyBiographyRecordKind, MembershipEventKind};
use conduit_core::SignId;
use std::net::TcpStream;

const MAX_BODY_MEMBERSHIP_EVIDENCE_BYTES: usize = 65_536;

mod reconciliation;

impl PatchbayHtmlServer {
    pub(super) fn deliver_body_membership_evidence(
        &mut self,
        stream: &mut TcpStream,
        bytes: &[u8],
    ) -> Result<(), ServerError> {
        let body = match self.apply_body_membership_evidence(bytes) {
            Ok(body) => body,
            Err(ServerError::InvalidRequest | ServerError::Interaction(_)) => {
                return super::write_response(
                    stream,
                    "400 Bad Request",
                    "text/plain; charset=utf-8",
                    b"invalid Body membership evidence",
                );
            }
            Err(error) => return Err(error),
        };
        super::write_response(stream, "200 OK", "application/json; charset=utf-8", &body)
    }

    pub(super) fn apply_body_membership_evidence(
        &mut self,
        bytes: &[u8],
    ) -> Result<Vec<u8>, ServerError> {
        if bytes.is_empty() || bytes.len() > MAX_BODY_MEMBERSHIP_EVIDENCE_BYTES {
            return Err(ServerError::InvalidRequest);
        }
        let candidate: BodyBiographyEvidence =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        candidate
            .validate()
            .map_err(|error| ServerError::Interaction(format!("invalid biography: {error:?}")))?;
        let prior_session = self
            .body_workload
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workload session is absent".into()))?;
        let candidate =
            reconciliation::merge_membership_extension(prior_session.evidence(), &candidate)?;
        let operation = validate_membership_extension(prior_session.evidence(), &candidate)?;
        let mut next_planning = self.body_planning.clone();
        if operation == "leave" {
            let record = candidate
                .records
                .last()
                .ok_or_else(|| ServerError::Interaction("Host leave record is absent".into()))?;
            let BodyBiographyRecordKind::HostLeft { part_id, .. } = &record.kind else {
                return Err(ServerError::Interaction(
                    "Host leave record is malformed".into(),
                ));
            };
            let leaving_host = prior_session
                .evidence()
                .membership
                .parts
                .iter()
                .find(|part| &part.part_id == part_id)
                .and_then(|part| part.current.as_ref())
                .map(|current| &current.host_id)
                .ok_or_else(|| ServerError::Interaction("leaving Host is not current".into()))?;
            if let Some(planning) = next_planning.as_mut() {
                let selected = planning.current_plan().forms.iter().any(|form| {
                    form.plan
                        .fragments
                        .iter()
                        .any(|fragment| &fragment.host_id == leaving_host)
                });
                if selected {
                    planning
                        .mark_current_unsatisfied(record.sign_id.clone())
                        .map_err(|error| {
                            ServerError::Interaction(format!("Body Host loss: {error:?}"))
                        })?;
                }
            }
        }

        let encoded = serde_json::to_vec(&candidate)
            .map_err(|error| ServerError::Interaction(error.to_string()))?;
        let session = patchbay_model::PatchbayBodyWorkloadSession::open_serialized(
            &encoded,
            crate::body_workbench::model_entrance(
                &self
                    .snapshot
                    .body_workbench
                    .as_ref()
                    .ok_or_else(|| ServerError::Interaction("Body workbench is absent".into()))?
                    .entrance,
            ),
        )
        .map_err(|error| ServerError::Interaction(format!("open updated biography: {error:?}")))?;
        let prior = self
            .snapshot
            .body_workbench
            .as_ref()
            .ok_or_else(|| ServerError::Interaction("Body workbench is absent".into()))?;
        let evidence_revision = prior
            .evidence_revision
            .checked_add(1)
            .ok_or_else(|| ServerError::Interaction("Body evidence revision exhausted".into()))?;
        let entrance = prior.entrance.clone();
        let reviewed_forms = prior.reviewed_forms.clone();
        let prior_interaction = self.snapshot.interaction.clone();
        let mut snapshot = crate::body_workbench::body_workbench_snapshot_with_reviewed(
            evidence_revision,
            &encoded,
            entrance,
            &reviewed_forms,
        )
        .map_err(|error| ServerError::Interaction(error.to_string()))?;
        snapshot.mark_available(SignId::from(format!(
            "patchbay-html/body-membership/evidence-{evidence_revision}/available"
        )))?;
        snapshot.interaction = prior_interaction;
        snapshot.interaction.revision = snapshot.interaction.revision.saturating_add(1);
        snapshot.interaction.last_request_id = Some(format!(
            "body-membership/{operation}/evidence-{evidence_revision}"
        ));
        snapshot.interaction.last_disposition = Some("Succeeded".into());
        snapshot.body_host_offer_evidence = self
            .snapshot
            .body_host_offer_evidence
            .clone()
            .filter(|evidence| super::body_host_offer_evidence::is_current(evidence, &candidate));
        snapshot.body_host_planning_offer = self
            .snapshot
            .body_host_planning_offer
            .clone()
            .filter(|evidence| super::body_host_offer_evidence::is_current(evidence, &candidate));
        snapshot.body_planning = next_planning.as_ref().map(|planning| planning.snapshot());
        let navigation = navigation_state(&snapshot)?;
        let encoded_snapshot = snapshot.encode()?;
        self.body_workload = Some(session);
        self.body_planning = next_planning;
        self.snapshot = snapshot;
        self.navigation = navigation;
        self.encoded_snapshot = encoded_snapshot;
        Ok(self.encoded_snapshot.clone())
    }
}

fn validate_membership_extension(
    prior: &BodyBiographyEvidence,
    candidate: &BodyBiographyEvidence,
) -> Result<&'static str, ServerError> {
    let prior_record_count = prior.records.len();
    let prior_event_count = prior.membership.events.len();
    let prior_part_count = prior.membership.parts.len();
    if candidate.schema != prior.schema
        || candidate.body_id != prior.body_id
        || candidate.friendly_name != prior.friendly_name
        || candidate.body != prior.body
        || candidate.wakes != prior.wakes
        || candidate.graduation != prior.graduation
        || candidate.records.get(..prior_record_count) != Some(prior.records.as_slice())
        || candidate.membership.events.get(..prior_event_count)
            != Some(prior.membership.events.as_slice())
    {
        return Err(ServerError::Interaction(
            "biography does not preserve prior Body truth and history".into(),
        ));
    }
    if candidate.records.len() == prior_record_count + 1
        && candidate.membership.events.len() == prior_event_count + 1
        && candidate.membership.parts.len() == prior_part_count
    {
        return validate_presence_extension(
            prior,
            candidate,
            prior_record_count,
            prior_event_count,
        );
    }
    if candidate.records.len() != prior_record_count + 2
        || candidate.membership.events.len() != prior_event_count + 2
        || candidate.membership.parts.len() != prior_part_count + 1
        || candidate.membership.parts.get(..prior_part_count)
            != Some(prior.membership.parts.as_slice())
    {
        return Err(ServerError::Interaction(
            "biography is not one bounded membership extension".into(),
        ));
    }
    let admitted_record = &candidate.records[prior_record_count];
    let joined_record = &candidate.records[prior_record_count + 1];
    let admitted_event = &candidate.membership.events[prior_event_count];
    let joined_event = &candidate.membership.events[prior_event_count + 1];
    let (
        BodyBiographyRecordKind::PartAdmitted {
            change_id: admitted_change,
            part_id: admitted_part,
        },
        BodyBiographyRecordKind::HostJoined {
            change_id: joined_change,
            part_id: joined_part,
            host_id,
            boot_id,
        },
        MembershipEventKind::Admitted { .. },
        MembershipEventKind::HostAttached { observation },
    ) = (
        &admitted_record.kind,
        &joined_record.kind,
        &admitted_event.kind,
        &joined_event.kind,
    )
    else {
        return Err(ServerError::Interaction(
            "biography extension is not Part admission followed by Host attachment".into(),
        ));
    };
    let part = &candidate.membership.parts[prior_part_count];
    if admitted_change != &admitted_event.change_id
        || joined_change != &joined_event.change_id
        || admitted_part != joined_part
        || admitted_part != &admitted_event.part_id
        || joined_part != &joined_event.part_id
        || part.part_id != *admitted_part
        || part.current.as_ref() != Some(observation)
        || &observation.host_id != host_id
        || &observation.boot_id != boot_id
    {
        return Err(ServerError::Interaction(
            "biography admission identities do not agree".into(),
        ));
    }
    Ok("admit")
}

fn validate_presence_extension(
    prior: &BodyBiographyEvidence,
    candidate: &BodyBiographyEvidence,
    record_index: usize,
    event_index: usize,
) -> Result<&'static str, ServerError> {
    let record = &candidate.records[record_index];
    let event = &candidate.membership.events[event_index];
    if let (
        BodyBiographyRecordKind::HostJoined {
            change_id,
            part_id,
            host_id,
            boot_id,
        },
        MembershipEventKind::HostAttached { observation },
    ) = (&record.kind, &event.kind)
    {
        let prior_part = prior
            .membership
            .parts
            .iter()
            .find(|candidate| candidate.part_id == *part_id)
            .ok_or_else(|| ServerError::Interaction("Host return names unknown Part".into()))?;
        let candidate_part = candidate
            .membership
            .parts
            .iter()
            .find(|candidate| candidate.part_id == *part_id)
            .ok_or_else(|| ServerError::Interaction("Host return removes durable Part".into()))?;
        let prior_host = prior
            .records
            .iter()
            .rev()
            .find_map(|record| match &record.kind {
                BodyBiographyRecordKind::HostJoined {
                    part_id: joined_part,
                    host_id,
                    ..
                } if joined_part == part_id => Some(host_id),
                _ => None,
            });
        let prior_boot = prior
            .records
            .iter()
            .rev()
            .find_map(|record| match &record.kind {
                BodyBiographyRecordKind::HostLeft {
                    part_id: left_part,
                    prior_boot_id,
                    ..
                } if left_part == part_id => Some(prior_boot_id),
                _ => None,
            });
        let other_parts_unchanged = prior.membership.parts.iter().all(|prior_part| {
            &prior_part.part_id == part_id
                || candidate
                    .membership
                    .parts
                    .iter()
                    .find(|candidate| candidate.part_id == prior_part.part_id)
                    == Some(prior_part)
        });
        if change_id != &event.change_id
            || part_id != &event.part_id
            || prior_part.current.is_some()
            || candidate_part.state != prior_part.state
            || candidate_part.current.as_ref() != Some(observation)
            || &observation.host_id != host_id
            || &observation.boot_id != boot_id
            || prior_host != Some(host_id)
            || prior_boot.is_none_or(|prior_boot| prior_boot == boot_id)
            || !other_parts_unchanged
        {
            return Err(ServerError::Interaction(
                "Host return identities do not agree".into(),
            ));
        }
        return Ok("return");
    }
    let (
        BodyBiographyRecordKind::HostLeft {
            change_id,
            part_id,
            prior_boot_id,
        },
        MembershipEventKind::HostDetached {
            prior_boot_id: event_boot,
        },
    ) = (&record.kind, &event.kind)
    else {
        return Err(ServerError::Interaction(
            "single-record membership extension is not Host leave or return".into(),
        ));
    };
    let prior_part = prior
        .membership
        .parts
        .iter()
        .find(|candidate| candidate.part_id == *part_id)
        .ok_or_else(|| ServerError::Interaction("Host leave names unknown Part".into()))?;
    let candidate_part = candidate
        .membership
        .parts
        .iter()
        .find(|candidate| candidate.part_id == *part_id)
        .ok_or_else(|| ServerError::Interaction("Host leave removes durable Part".into()))?;
    let other_parts_unchanged = prior.membership.parts.iter().all(|prior_part| {
        &prior_part.part_id == part_id
            || candidate
                .membership
                .parts
                .iter()
                .find(|candidate| candidate.part_id == prior_part.part_id)
                == Some(prior_part)
    });
    if change_id != &event.change_id
        || part_id != &event.part_id
        || prior_boot_id != event_boot
        || prior_part.current.as_ref().map(|current| &current.boot_id) != Some(prior_boot_id)
        || candidate_part.state != prior_part.state
        || candidate_part.current.is_some()
        || !other_parts_unchanged
    {
        return Err(ServerError::Interaction(
            "Host leave identities do not agree".into(),
        ));
    }
    Ok("leave")
}

#[cfg(test)]
#[path = "body_membership_evidence_tests.rs"]
mod tests;
