use super::*;

impl BodyBiographyEvidence {
    pub fn validate(&self) -> Result<(), BodyBiographyError> {
        self.body
            .validate()
            .map_err(|_| BodyBiographyError::InvalidBody)?;
        self.membership
            .validate()
            .map_err(|_| BodyBiographyError::InvalidEvidence)?;
        if self.schema != "conduit.body/biography-evidence@2"
            || self.body_id != self.body.body_id
            || self.body_id != self.membership.body_id
            || self.friendly_name.trim().is_empty()
            || self.friendly_name.len() > MAX_BODY_FRIENDLY_NAME_BYTES
        {
            return Err(BodyBiographyError::InvalidMetadata);
        }
        if let Some(graduation) = &self.graduation {
            validate_graduation(graduation)?;
        }
        self.validate_records()?;
        self.validate_wake_history()
    }

    pub(super) fn validate_records(&self) -> Result<(), BodyBiographyError> {
        if self.records.is_empty() || self.records.len() > MAX_BODY_BIOGRAPHY_RECORDS {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        let first = &self.records[0];
        if first.sequence != self.body.birth_sequence
            || !matches!(&first.kind, BodyBiographyRecordKind::Born { initial_workset, workload_revision: 0 }
                if self.body.events.first().is_some_and(|event| matches!(event,
                    BodyLifecycleEvent::Born { initial_workset: body_initial, workload_revision: 0, .. }
                    if body_initial == initial_workset)))
            || first.sign_id != self.body.sign_ids[0]
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        if self
            .records
            .windows(2)
            .any(|pair| pair[0].sequence >= pair[1].sequence)
        {
            return Err(BodyBiographyError::InvalidSequence);
        }
        for record in self.records.iter().skip(1) {
            match &record.kind {
                BodyBiographyRecordKind::WakeEvent { .. }
                | BodyBiographyRecordKind::LullRetained { .. } => {}
                BodyBiographyRecordKind::PartAdmitted { change_id, part_id } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(event.kind, MembershipEventKind::Admitted { .. })
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::HostJoined {
                    change_id,
                    part_id,
                    host_id,
                    boot_id,
                } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(&event.kind, MembershipEventKind::HostAttached { observation } if &observation.host_id == host_id && &observation.boot_id == boot_id)
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::HostLeft {
                    change_id,
                    part_id,
                    prior_boot_id,
                } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(&event.kind, MembershipEventKind::HostDetached { prior_boot_id: actual } if actual == prior_boot_id)
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::PartRevoked { change_id, part_id } => {
                    let event = membership_event(&self.membership, change_id)?;
                    if event.part_id != *part_id
                        || event.sign_id != record.sign_id
                        || !matches!(event.kind, MembershipEventKind::Revoked)
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::FormAdmitted {
                    source_document_id,
                    checked_form_id,
                    workload_revision,
                }
                | BodyBiographyRecordKind::FormRemoved {
                    source_document_id,
                    checked_form_id,
                    workload_revision,
                } => {
                    let event = self.body.events.iter().find(|event| {
                        event.sign_id() == &record.sign_id
                            && match event {
                                BodyLifecycleEvent::FormAdmitted {
                                    source_document_id: source,
                                    checked_form_id: checked,
                                    workload_revision: revision,
                                    ..
                                } if matches!(
                                    record.kind,
                                    BodyBiographyRecordKind::FormAdmitted { .. }
                                ) =>
                                {
                                    source == source_document_id
                                        && checked == checked_form_id
                                        && revision == workload_revision
                                }
                                BodyLifecycleEvent::FormRemoved {
                                    source_document_id: source,
                                    checked_form_id: checked,
                                    workload_revision: revision,
                                    ..
                                } if matches!(
                                    record.kind,
                                    BodyBiographyRecordKind::FormRemoved { .. }
                                ) =>
                                {
                                    source == source_document_id
                                        && checked == checked_form_id
                                        && revision == workload_revision
                                }
                                _ => false,
                            }
                    });
                    if event.is_none() {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::Graduated {
                    choice,
                    patchbay_plan_id,
                    patchbay_implementation_id,
                } => {
                    let graduation = self
                        .graduation
                        .as_ref()
                        .ok_or(BodyBiographyError::InvalidEvidence)?;
                    if graduation.sequence != record.sequence
                        || graduation.sign_id != record.sign_id
                        || &graduation.choice != choice
                        || &graduation.patchbay_plan_id != patchbay_plan_id
                        || &graduation.patchbay_implementation_id != patchbay_implementation_id
                    {
                        return Err(BodyBiographyError::InvalidEvidence);
                    }
                }
                BodyBiographyRecordKind::Born { .. } => {
                    return Err(BodyBiographyError::DuplicateEvidence)
                }
            }
        }
        let membership_records = self
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    BodyBiographyRecordKind::PartAdmitted { .. }
                        | BodyBiographyRecordKind::HostJoined { .. }
                        | BodyBiographyRecordKind::HostLeft { .. }
                        | BodyBiographyRecordKind::PartRevoked { .. }
                )
            })
            .count();
        let workload_records = self
            .records
            .iter()
            .filter(|record| {
                matches!(
                    record.kind,
                    BodyBiographyRecordKind::FormAdmitted { .. }
                        | BodyBiographyRecordKind::FormRemoved { .. }
                )
            })
            .count();
        let workload_events = self
            .body
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    BodyLifecycleEvent::FormAdmitted { .. }
                        | BodyLifecycleEvent::FormRemoved { .. }
                )
            })
            .count();
        if membership_records != self.membership.events.len()
            || workload_records != workload_events
            || self.graduation.is_some()
                != self
                    .records
                    .iter()
                    .any(|record| matches!(record.kind, BodyBiographyRecordKind::Graduated { .. }))
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }
}
