//! The native carrier calls the same durable owner withdrawal as browser Masks.
use super::*;
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_member_leave(
        &mut self,
        credential: &MembershipCredential,
    ) -> Result<BodyBiographyEvidence, String> {
        let HostSource::Body { owner, root, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        owner.member_leave(root, credential)
    }
}
#[cfg(unix)]
pub(crate) fn leave_member(
    state_dir: &Path,
    credential: MembershipCredential,
) -> Result<(), String> {
    match call(
        state_dir,
        Request::BodyMemberLeave {
            protocol: PROTOCOL,
            token: token(state_dir)?,
            credential,
        },
    )? {
        Response::BodyMemberLeft { protocol: PROTOCOL } => Ok(()),
        Response::Refused { code, .. } => Err(code),
        _ => Err("Body owner returned the wrong member leave response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn leave_member(
    _state_dir: &Path,
    _credential: MembershipCredential,
) -> Result<(), String> {
    Err("installed member leave requires local owner control".into())
}
