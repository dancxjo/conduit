//! Existing emergency configuration transitions, kept separate from biography storage.
use super::*;

impl BodyBiographyEvidence {
    pub fn born_with_emergency(
        body: Body,
        membership: BodyMembership,
        friendly_name: String,
        emergency: crate::DurableEmergencyConfiguration,
        configuration_sequence: u64,
        configuration_sign_id: SignId,
    ) -> Result<Self, BodyBiographyError> {
        emergency
            .validate()
            .map_err(|_| BodyBiographyError::InvalidMetadata)?;
        let mut evidence = Self::born(body, membership, friendly_name)?;
        if emergency.revision != 1
            || configuration_sequence <= evidence.body.birth_sequence
            || configuration_sign_id.as_str().is_empty()
            || configuration_sign_id.as_str().len() > MAX_LIFECYCLE_ID_BYTES
        {
            return Err(BodyBiographyError::InvalidMetadata);
        }
        evidence.records.push(BodyBiographyRecord {
            sequence: configuration_sequence,
            sign_id: configuration_sign_id,
            kind: BodyBiographyRecordKind::EmergencyConfigured {
                configuration: emergency.clone(),
            },
        });
        evidence.emergency = Some(emergency);
        evidence.validate()?;
        Ok(evidence)
    }

    pub fn reconfigure_emergency(
        &mut self,
        emergency: crate::DurableEmergencyConfiguration,
        sequence: u64,
        sign_id: SignId,
    ) -> Result<(), BodyBiographyError> {
        if matches!(self.body.state, crate::BodyState::Fulfilled { .. }) {
            return Err(BodyBiographyError::BodyFulfilled);
        }
        emergency
            .validate()
            .map_err(|_| BodyBiographyError::InvalidMetadata)?;
        let expected_revision = self
            .emergency
            .as_ref()
            .map_or(1, |current| current.revision.saturating_add(1));
        if emergency.revision != expected_revision
            || sequence <= self.last_sequence()
            || sign_id.as_str().is_empty()
            || sign_id.as_str().len() > MAX_LIFECYCLE_ID_BYTES
            || self.records.iter().any(|record| record.sign_id == sign_id)
        {
            return Err(BodyBiographyError::InvalidSequence);
        }
        self.can_append(1)?;
        let mut candidate = self.clone();
        candidate.records.push(BodyBiographyRecord {
            sequence,
            sign_id,
            kind: BodyBiographyRecordKind::EmergencyConfigured {
                configuration: emergency.clone(),
            },
        });
        candidate.emergency = Some(emergency);
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }
}
