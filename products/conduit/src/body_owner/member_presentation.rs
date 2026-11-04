//! Descriptive member presentation claim, never possession or route authority.
//! Every owner use must recheck current admitted Part, latest credential,
//! observed offer and actual admitted Lines. Construction authenticates nothing.
use conduit_body::MembershipCredential;
use conduit_core::{HostAdvertisement, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct MemberPresentationBasis {
    pub(crate) credential: MembershipCredential,
    pub(crate) host_advertisement: HostAdvertisement,
}
impl MemberPresentationBasis {
    /// Validate only bounded identity and internal coherence. The caller must
    /// independently verify a returning member before using this claim.
    pub(crate) fn new(
        credential: MembershipCredential,
        host_advertisement: HostAdvertisement,
    ) -> Result<Self, String> {
        let claim = Self {
            credential,
            host_advertisement,
        };
        claim.validate()?;
        Ok(claim)
    }

    /// Recheck deserialized claims without cloning or allocating on success.
    /// Validation is internal coherence only, never authenticated membership.
    pub(crate) fn validate(&self) -> Result<(), String> {
        let credential = &self.credential;
        let host_advertisement = &self.host_advertisement;
        let ids = [
            credential.credential_id.as_str(),
            credential.body_id.as_str(),
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            host_advertisement.profile.as_str(),
        ];
        if ids.iter().any(|id| id.is_empty() || id.len() > 256)
            || credential.host_id != host_advertisement.host_id
            || credential.boot_id != host_advertisement.boot_id
            || host_advertisement.protocol_version != PROTOCOL_VERSION
            || host_advertisement.offer_generation.0 == 0
        {
            return Err("presentation claim has invalid member Host scope".into());
        }
        Ok(())
    }
}
