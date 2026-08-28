use sha2::{Digest, Sha256};

use crate::{HostProfile, ProfileDiagnostic};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn canonical_profile_json(profile: &HostProfile) -> Result<Vec<u8>, ProfileDiagnostic> {
    let mut canonical = profile.clone();
    canonical.fragments.sort();
    canonical.capabilities.sort_by(|left, right| {
        (&left.kind, &left.contract_revision, &left.implementation).cmp(&(
            &right.kind,
            &right.contract_revision,
            &right.implementation,
        ))
    });
    canonical.host_operations.sort();
    canonical
        .resources
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .bases
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical
        .drivers
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.lines.sort();
    canonical
        .presenters
        .sort_by(|left, right| left.id.cmp(&right.id));
    canonical.facilities.sort();
    canonical.exclusions.sort();
    serde_json::to_vec(&canonical).map_err(|error| ProfileDiagnostic::Encoding {
        detail: error.to_string(),
    })
}

pub(crate) fn profile_id(profile: &HostProfile) -> Result<ProfileId, ProfileDiagnostic> {
    let canonical = canonical_profile_json(profile)?;
    let digest = Sha256::digest(canonical);
    Ok(ProfileId(format!("sha256:{digest:x}")))
}
