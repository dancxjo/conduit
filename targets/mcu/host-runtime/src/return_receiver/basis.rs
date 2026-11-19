use super::Refusal;
use conduit_body::{
    FederationPeer, MemberReturnImagePresentation, MembershipCredential, MAX_LIFECYCLE_ID_BYTES,
};
use conduit_core::{HostProfileId, PROTOCOL_VERSION};
use ed25519_dalek::VerifyingKey;
/// Independently retained public pins, never obtained from the presentation.
#[derive(Clone, Copy)]
pub struct Basis<'a> {
    pub credential: &'a MembershipCredential,
    pub expected_profile: &'a HostProfileId,
    pub member_verifying_key: &'a [u8; 32],
    pub owner: &'a FederationPeer,
}
impl Basis<'_> {
    pub fn validate(self) -> Result<(), Refusal> {
        for id in [
            self.credential.credential_id.as_str(),
            self.credential.body_id.as_str(),
            self.credential.part_id.as_str(),
            self.credential.host_id.as_str(),
            self.credential.boot_id.as_str(),
            self.expected_profile.as_str(),
            self.owner.host_id.as_str(),
            self.owner.boot_id.as_str(),
        ] {
            if !finite(id) {
                return Err(Refusal::Scope);
            }
        }
        if self.owner.offer_generation.0 == 0 {
            return Err(Refusal::Scope);
        }
        for key in [self.member_verifying_key, &self.owner.verifying_key] {
            if *key == [0; 32] || VerifyingKey::from_bytes(key).is_err() {
                return Err(Refusal::Scope);
            }
        }
        Ok(())
    }
    pub(super) fn check(self, presentation: &MemberReturnImagePresentation) -> Result<(), Refusal> {
        self.validate()?;
        presentation
            .verify(self.member_verifying_key)
            .map_err(|_| Refusal::Signature)?;
        presentation
            .projection
            .validate()
            .map_err(|_| Refusal::Scope)?;
        let actual = &presentation.projection.context;
        if presentation.credential != *self.credential
            || actual.host_id != self.credential.host_id
            || actual.profile != *self.expected_profile
            || actual.protocol_version != PROTOCOL_VERSION
            || actual.offer_generation.0 == 0
            || !finite(actual.boot_id.as_str())
            || !conduit_core::bounded_host_advertisement_comparison_supported(actual)
        {
            return Err(Refusal::Scope);
        }
        Ok(())
    }
}
pub(super) fn finite(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_LIFECYCLE_ID_BYTES
        && !value.chars().any(char::is_control)
}
