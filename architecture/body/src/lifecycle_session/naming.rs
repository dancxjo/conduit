use super::*;
use crate::{
    AuthorizedBodyRename, BodyBiographyRecord, BodyBiographyRecordKind, BodyRenameRefusal,
    BodyRenameResult,
};

impl BodyLifecycleSession {
    /// Authorize at the trusted boundary first; persist the resulting session
    /// atomically before publishing this result as a successful rename.
    pub fn rename(
        &mut self,
        authorized: &AuthorizedBodyRename,
        host: &HostId,
        boot: &BootId,
    ) -> Result<BodyRenameResult, BodyRenameRefusal> {
        let request = &authorized.request;
        self.require_mutable()
            .map_err(|_| BodyRenameRefusal::Fulfilled)?;
        self.require_host(host, boot)
            .map_err(|_| BodyRenameRefusal::Host)?;
        if request.body_id != self.evidence.body_id {
            return Err(BodyRenameRefusal::WrongBody);
        }
        if &request.host_id != host || &request.boot_id != boot {
            return Err(BodyRenameRefusal::Host);
        }
        let retry = self
            .evidence
            .records
            .iter()
            .rev()
            .find(|record| matches!(record.kind, BodyBiographyRecordKind::Renamed { .. }));
        if self.evidence.friendly_name == request.new_name {
            if let Some(record) = retry.filter(|record| matches!(&record.kind, BodyBiographyRecordKind::Renamed { request_digest, .. } if request_digest == &authorized.digest)) {
                return Ok(BodyRenameResult { body_id: self.evidence.body_id.clone(), friendly_name: self.evidence.friendly_name.clone(), sequence: record.sequence, sign_id: record.sign_id.clone(), request_digest: authorized.digest, record_digest: Some(record.rename_digest().map_err(|_| BodyRenameRefusal::Evidence)?), changed: false });
            }
        }
        if request.expected_sequence != self.evidence.last_sequence() {
            return Err(BodyRenameRefusal::Stale);
        }
        let changed = self.evidence.friendly_name != request.new_name;
        let sequence = if changed {
            self.next_sequence()
                .map_err(|_| BodyRenameRefusal::Capacity)?
        } else {
            self.evidence.last_sequence()
        };
        let sign_id = if changed {
            sign(host, boot, sequence)
        } else {
            self.evidence
                .records
                .last()
                .ok_or(BodyRenameRefusal::Evidence)?
                .sign_id
                .clone()
        };
        // Prepare completion storage before changing the authoritative session.
        let mut result = BodyRenameResult {
            body_id: self.evidence.body_id.clone(),
            friendly_name: request.new_name.clone(),
            sequence,
            sign_id: sign_id.clone(),
            request_digest: authorized.digest,
            record_digest: None,
            changed,
        };
        if changed {
            self.evidence
                .can_append(1)
                .map_err(|_| BodyRenameRefusal::Capacity)?;
            let mut candidate = self.evidence.clone();
            candidate.schema = "conduit.body/biography-evidence@3".into();
            candidate.records.push(BodyBiographyRecord {
                sequence,
                sign_id: sign_id.clone(),
                kind: BodyBiographyRecordKind::Renamed {
                    previous_name: candidate.friendly_name.clone(),
                    new_name: request.new_name.clone(),
                    owner_key: authorized.owner_key,
                    attribution: request.attribution.clone(),
                    request_digest: authorized.digest,
                },
            });
            candidate.friendly_name = request.new_name.clone();
            candidate
                .validate()
                .map_err(|_| BodyRenameRefusal::Evidence)?;
            result.record_digest = Some(
                candidate
                    .records
                    .last()
                    .ok_or(BodyRenameRefusal::Evidence)?
                    .rename_digest()
                    .map_err(|_| BodyRenameRefusal::Evidence)?,
            );
            self.evidence = candidate;
        }
        Ok(result)
    }
}
