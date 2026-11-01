use super::*;

impl BodyBiographyEvidence {
    pub(super) fn validate_names(&self) -> Result<(), BodyBiographyError> {
        let mut last_name: Option<&str> = None;
        for record in &self.records {
            if let BodyBiographyRecordKind::Renamed {
                previous_name,
                new_name,
                owner_key,
                attribution,
                request_digest,
            } = &record.kind
            {
                if self.schema != "conduit.body/biography-evidence@3"
                    || !valid_name(previous_name)
                    || !valid_name(new_name)
                    || previous_name == new_name
                    || *owner_key == [0; 32]
                    || *request_digest == [0; 32]
                    || attribution.trim().is_empty()
                    || attribution.len() > MAX_LIFECYCLE_ID_BYTES
                    || attribution.chars().any(char::is_control)
                    || last_name.is_some_and(|name| name != previous_name)
                    || record.sign_id.as_str().is_empty()
                {
                    return Err(BodyBiographyError::InvalidEvidence);
                }
                last_name = Some(new_name);
            }
        }
        if last_name.is_some_and(|name| name != self.friendly_name)
            || (self.schema == "conduit.body/biography-evidence@3" && last_name.is_none())
        {
            return Err(BodyBiographyError::InvalidEvidence);
        }
        Ok(())
    }

    /// Archive all but the latest rename, which anchors the resident current name.
    /// The returned segment and replacement evidence require one durable commit.
    pub fn seal_name_history(
        &mut self,
    ) -> Result<Option<BodyBiographyArchiveSegment>, BodyBiographyError> {
        self.validate()?;
        let names: Vec<_> = self
            .records
            .iter()
            .filter(|record| matches!(record.kind, BodyBiographyRecordKind::Renamed { .. }))
            .cloned()
            .collect();
        if names.len() < 2 {
            return Ok(None);
        }
        let records = names[..names.len() - 1].to_vec();
        let segment = BodyBiographyArchiveSegment::seal(
            self,
            records.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )?;
        let mut candidate = self.clone();
        candidate
            .records
            .retain(|record| !records.iter().any(|old| old.sequence == record.sequence));
        candidate.advance_compaction(&segment, &records, 0, None)?;
        candidate.validate()?;
        *self = candidate;
        Ok(Some(segment))
    }
}

fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= MAX_BODY_FRIENDLY_NAME_BYTES
        && !name.chars().any(char::is_control)
}
