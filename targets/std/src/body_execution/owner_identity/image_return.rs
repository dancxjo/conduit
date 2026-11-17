//! Typed attestation through the already prepared original owner identity.
use super::StartupOwnerIdentity;
use conduit_body::{
    attest_member_return_image_acknowledgement, CurrentMemberContinuity,
    MemberReturnImageAcknowledgement, SignedPortableAdmissionReceipt,
};
impl StartupOwnerIdentity {
    /// Authenticate an exact image-return receipt for a manager-observed current
    /// member, deriving the member key solely from that retained continuity.
    /// This does not commit membership or prove receipt persistence. The product
    /// must bind the full projection/grants to its original pending transaction,
    /// finish durable Core completion, recheck its current owner identity/latest
    /// credential, and retain the signed receipt before carrier delivery.
    /// No new key, arbitrary byte signature or native effect authority is issued.
    pub fn attest_image_member_return(
        &self,
        receipt: &MemberReturnImageAcknowledgement,
        member: &CurrentMemberContinuity,
    ) -> Result<SignedPortableAdmissionReceipt, String> {
        let credential = &receipt.credential;
        let actual = &receipt.projection.context;
        let peer = member.peer();
        if &credential.body_id != member.body_id()
            || &credential.part_id != member.part_id()
            || credential.host_id != peer.host_id
            || credential.boot_id != peer.boot_id
            || actual.host_id != peer.host_id
            || actual.boot_id != peer.boot_id
            || actual.offer_generation != peer.offer_generation
        {
            return Err("image return differs from original current-member continuity".into());
        }
        attest_member_return_image_acknowledgement(
            receipt,
            self.peer.clone(),
            peer.verifying_key,
            |transcript| self.key.sign(transcript),
        )
        .map_err(|error| format!("typed image-return attestation refused: {error:?}"))
    }
}
#[cfg(test)]
mod tests;
