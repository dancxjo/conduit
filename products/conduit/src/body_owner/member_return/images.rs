//! Image-return admission is unavailable until original IMAGE workspace custody
//! covers both transient preparation and retained pending/unknown-ACK owners.
//! A fallible Vec allocation or the prepared identity lease is not that grant.
use super::*;
use conduit_body::{MemberReturnImagePresentation, SignedPortableAdmissionReceipt};
const WORKSPACE_REFUSAL: &str = "member-image-return-workspace-not-admitted";
impl Owner {
    pub(crate) fn begin_image_member_return(
        &mut self,
        _presentation: MemberReturnImagePresentation,
        _images: &[Vec<u8>],
    ) -> Result<PartReturnChallenge, String> {
        // No grant exists for any production profile yet, including unbounded
        // Hosted profiles. Never decode offers or mutate admission as fallback.
        Err(WORKSPACE_REFUSAL.into())
    }
    pub(crate) fn complete_image_member_return(
        &mut self,
        _root: &Path,
        _proof: &PartReturnProof,
    ) -> Result<SignedPortableAdmissionReceipt, String> {
        Err(WORKSPACE_REFUSAL.into())
    }
}
// Transaction-ordering fixtures are not an installed admission entrance.
#[cfg(test)]
#[path = "images/fixture.rs"]
pub(crate) mod fixture;
