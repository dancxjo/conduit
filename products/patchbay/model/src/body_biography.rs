use conduit_body::BodyId;
use conduit_body::{
    BodyBiographyEvidence, BodyBiographyRecordKind, BodyGraduationChoice,
    MAX_BODY_BIOGRAPHY_RECORDS,
};
use conduit_core::SignId;

pub const MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyBiographyProjection {
    pub schema: &'static str,
    pub body_id: BodyId,
    pub friendly_name: String,
    pub entries: Vec<BodyBiographyEntry>,
    pub archived_history: Option<BodyBiographyArchiveProjection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyBiographyArchiveProjection {
    pub sealed_segments: u64,
    pub wakes: u64,
    pub records: u64,
    pub through_sequence: u64,
    pub head_digest: Option<[u8; 32]>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyBiographyEntry {
    pub sequence: u64,
    pub heading: &'static str,
    pub explanation: String,
    pub evidence_sign_id: SignId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyBiographyProjectionError {
    InvalidEvidence,
    ExplanationTooLong,
}

pub fn project_body_biography(
    evidence: &BodyBiographyEvidence,
) -> Result<BodyBiographyProjection, BodyBiographyProjectionError> {
    evidence
        .validate()
        .map_err(|_| BodyBiographyProjectionError::InvalidEvidence)?;
    let mut entries = Vec::with_capacity(evidence.records.len().min(MAX_BODY_BIOGRAPHY_RECORDS));
    for record in &evidence.records {
        let (heading, explanation) = match &record.kind {
            BodyBiographyRecordKind::WakeEvent { wake_id, event_index } => {
                let wake = evidence.wakes.iter().find(|wake| &wake.wake_id == wake_id)
                    .expect("validated Wake reference");
                let heading = wake_event_heading(&wake.events[usize::from(*event_index)]);
                (heading, format!("{} in Wake {} (event {}).", heading, wake_id.as_str(), event_index))
            }
            BodyBiographyRecordKind::LullRetained { wake_id } => (
                "Body retained after Lull",
                format!("Wake {} ended; the Body and its history remain available.", wake_id.as_str()),
            ),
            BodyBiographyRecordKind::Born { initial_workset, workload_revision } => (
                "Born",
                format!(
                    "{} became Body {} with {} initial active Form(s) at workload revision {}.",
                    evidence.friendly_name,
                    evidence.body_id.as_str(),
                    initial_workset.len(),
                    workload_revision,
                ),
            ),
            BodyBiographyRecordKind::PartAdmitted { part_id, .. } => (
                "Part admitted",
                format!(
                    "Part {} joined the admitted membership of this Body.",
                    part_id.as_str()
                ),
            ),
            BodyBiographyRecordKind::HostJoined {
                part_id,
                host_id,
                boot_id,
                ..
            } => (
                "Host joined",
                format!(
                    "Part {} was observed on Host {}, Boot {}.",
                    part_id.as_str(),
                    host_id.as_str(),
                    boot_id.as_str()
                ),
            ),
            BodyBiographyRecordKind::HostLeft {
                part_id,
                prior_boot_id,
                ..
            } => (
                "Host left",
                format!(
                    "Part {} left its prior Boot {}; the Part remains admitted.",
                    part_id.as_str(),
                    prior_boot_id.as_str()
                ),
            ),
            BodyBiographyRecordKind::PartRevoked { part_id, .. } => (
                "Part revoked",
                format!("Part {} was removed from Body membership.", part_id.as_str()),
            ),
            BodyBiographyRecordKind::FormAdmitted {
                checked_form_id,
                workload_revision,
                ..
            } => (
                "Form admitted",
                format!(
                    "Form {} joined the Body workset at revision {}.",
                    checked_form_id.as_str(),
                    workload_revision
                ),
            ),
            BodyBiographyRecordKind::FormRemoved {
                checked_form_id,
                workload_revision,
                ..
            } => (
                "Form stopped",
                format!(
                    "Form {} left the Body workset at revision {} without deleting the Body.",
                    checked_form_id.as_str(),
                    workload_revision
                ),
            ),
            BodyBiographyRecordKind::Fulfilled {
                final_workload_revision,
                attribution,
                settled_obligations,
                ..
            } => (
                "Fulfilled",
                format!(
                    "This Body completed its useful life at workload revision {} by {} after {} settlement obligation(s). Its biography and evidence remain available.",
                    final_workload_revision,
                    attribution,
                    settled_obligations.len(),
                ),
            ),
            BodyBiographyRecordKind::Graduated {
                choice: BodyGraduationChoice::HostedPatchbay,
                patchbay_plan_id,
                patchbay_implementation_id,
            } => (
                "Graduated from the Crèche",
                format!(
                    "Patchbay was placed by Plan {} using implementation {}. The durable Body evidence remains independent of this reader.",
                    patchbay_plan_id
                        .as_ref()
                        .expect("validated hosted graduation")
                        .as_str(),
                    patchbay_implementation_id
                        .as_ref()
                        .expect("validated hosted graduation")
                        .as_str()
                ),
            ),
            BodyBiographyRecordKind::Graduated {
                choice: BodyGraduationChoice::ExternalReader,
                ..
            } => (
                "Graduated from the Crèche",
                "No Patchbay was hosted. A compatible reader can project this same durable Body evidence later."
                    .into(),
            ),
            BodyBiographyRecordKind::EmergencyConfigured { configuration } => {
                let words = configuration
                    .key
                    .words()
                    .expect("validated emergency key vocabulary");
                (
                    "Emergency phrase configured",
                    format!(
                        "Emergency phrase revision {} is {} {} {}. Detector {} reports {:?}; entropy came from admitted provider {}.",
                        configuration.revision,
                        words[0],
                        words[1],
                        words[2],
                        configuration.detector_version,
                        configuration.acoustic_availability,
                        configuration.entropy_provider_id,
                    ),
                )
            }
        };
        if explanation.len() > MAX_BODY_BIOGRAPHY_EXPLANATION_BYTES {
            return Err(BodyBiographyProjectionError::ExplanationTooLong);
        }
        entries.push(BodyBiographyEntry {
            sequence: record.sequence,
            heading,
            explanation,
            evidence_sign_id: record.sign_id.clone(),
        });
    }
    Ok(BodyBiographyProjection {
        schema: "conduit.patchbay/body-biography-projection@1",
        body_id: evidence.body_id.clone(),
        friendly_name: evidence.friendly_name.clone(),
        entries,
        archived_history: evidence.compaction.as_ref().map(|summary| BodyBiographyArchiveProjection {
            sealed_segments: summary.sealed_segments,
            wakes: summary.wakes,
            records: summary.records,
            through_sequence: summary.through_sequence,
            head_digest: summary.archive_head_digest,
            explanation: format!(
                "{} older Wake(s) and {} exact record(s) are sealed in {} integrity-linked history segment(s) through evidence sequence {}; load the exact head digest to inspect older evidence on demand.",
                summary.wakes, summary.records, summary.sealed_segments, summary.through_sequence
            ),
        }),
    })
}

fn wake_event_heading(event: &conduit_body::WakeLifecycleEvent) -> &'static str {
    use conduit_body::WakeLifecycleEvent::*;
    match event {
        Woke { .. } => "Woke",
        PlanReady { .. } => "Plan ready",
        PlanHeld { .. } => "Plan held",
        HeldPlanReleased { .. } => "Held Plan released",
        HeldPlanInvalidated { .. } => "Held Plan invalidated",
        PlayStarted { .. } => "Play started",
        BecameUnsatisfied { .. } => "Plan unsatisfied",
        WorkloadChanged { .. } => "Wake workload changed",
        Replanned { .. } => "Replacement Plan accepted",
        SamePlanObserved { .. } => "Same Plan observed",
        Lulled { .. } => "Wake lulled",
        Failed { .. } => "Wake failed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_body::{
        Body, BodyBiographyEvidence, BodyMembership, DurableEmergencyConfiguration,
        EmergencyAcousticAvailability, EmergencyKey,
    };
    use conduit_core::{bind_sign, BootId, CheckedFormId, HostId, SourceDocumentId};

    #[test]
    fn projects_only_valid_durable_birth_evidence_after_roundtrip() {
        let host = HostId::from("host/biography-reader");
        let boot = BootId::from("boot/biography-reader");
        let sign = bind_sign(&host, &boot, None, 7);
        let body = Body::born(
            SourceDocumentId::from("source/biography-reader"),
            CheckedFormId::from("checked/biography-reader"),
            7,
            sign.sign_id,
        )
        .unwrap();
        let evidence = BodyBiographyEvidence::born(
            body.clone(),
            BodyMembership::new(body.body_id.clone()).unwrap(),
            "Workbench".into(),
        )
        .unwrap();
        let reopened: BodyBiographyEvidence =
            serde_json::from_str(&serde_json::to_string(&evidence).unwrap()).unwrap();

        let projection = project_body_biography(&reopened).unwrap();
        assert_eq!(projection.body_id, body.body_id);
        assert_eq!(projection.entries.len(), 1);
        assert_eq!(projection.entries[0].heading, "Born");

        let mut invented = reopened;
        invented.records[0].sequence += 1;
        assert_eq!(
            project_body_biography(&invented),
            Err(BodyBiographyProjectionError::InvalidEvidence)
        );
    }

    #[test]
    fn projects_the_exact_durable_emergency_phrase_and_detector_state() {
        let body = Body::born(
            SourceDocumentId::from("source/emergency-reader"),
            CheckedFormId::from("checked/emergency-reader"),
            1,
            SignId::from("sign/emergency-reader-born"),
        )
        .unwrap();
        let membership = BodyMembership::new(body.body_id.clone()).unwrap();
        let evidence = BodyBiographyEvidence::born_with_emergency(
            body,
            membership,
            "Emergency reader".into(),
            DurableEmergencyConfiguration {
                revision: 1,
                key: EmergencyKey::from_admitted_entropy([0, 1, 2]).unwrap(),
                detector_version: "fixed-keyword-spotter/en-us@1".into(),
                entropy_provider_id: "base/entropy/specimen".into(),
                acoustic_availability: EmergencyAcousticAvailability::Ready,
            },
            2,
            SignId::from("sign/emergency-reader-configured"),
        )
        .unwrap();

        let projection = project_body_biography(&evidence).unwrap();
        assert_eq!(projection.entries.len(), 2);
        assert_eq!(projection.entries[1].heading, "Emergency phrase configured");
        assert!(projection.entries[1]
            .explanation
            .contains("copper kestrel lantern"));
        assert!(projection.entries[1]
            .explanation
            .contains("fixed-keyword-spotter/en-us@1 reports Ready"));
    }
}
