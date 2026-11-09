//! Compact affine append equipment publication; no decoded schema or grant.
use super::*;
use conduit_resource_snapshot::{
    append::PreparedNorRecordResidence,
    native_append::{
        AppendNativeIdentity, ReadyRecordAppender, APPEND_CAPABILITY, APPEND_CLASS,
        APPEND_MECHANISM,
    },
    nor::NorStorage,
};
fn project_append(
    identity: &AppendNativeIdentity,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    let mut facts = facts!(identity, APPEND_CLASS, APPEND_CAPABILITY);
    facts.mechanism = APPEND_MECHANISM;
    project(facts, actual, maximum_bytes)
}
/// Actual Ready custody only, including its current physical recovery state.
/// One plain unit owns the entire separately funded finite residence; it is not
/// unlimited storage, a content Resource reference, write permission or Start.
pub fn record_append<S: NorStorage>(
    owner: &ReadyRecordAppender<S>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    let attachment = owner.attachment();
    if !owner.recovered() || attachment.slots == 0 || attachment.resource_bytes == 0 {
        return Err(Refusal::Context);
    }
    project_append(owner.native_identity(), actual, maximum_bytes)
}
/// Before original grant issuance, publish only the initialized owned equipment.
/// # Safety
/// Root exclusively owns this exact recovered physical residence paired with
/// this independently initialized current identity/generation, not incoming
/// assignment or copied readiness metadata. Root must consume THESE SAME
/// original owners into ReadyRecordAppender after ordinary Issuer authorization.
/// The full finite extent, metadata/workspace and lawful execution windows need
/// separate admission. Recovery and public metadata confer no writable authority.
pub unsafe fn recovered_record_append<S: NorStorage>(
    identity: &AppendNativeIdentity,
    residence: &PreparedNorRecordResidence<S>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    // SAFETY: forwards the caller's exact original physical-pair obligations;
    // this checked borrow is dropped without IO, grant creation or mutation.
    unsafe { crate::execution_authority::NativeFacts::recovered_append(identity, residence) }
        .map_err(|_| Refusal::Context)?;
    project_append(identity, actual, maximum_bytes)
}
impl Publication<'_> {
    pub fn record_append<S: NorStorage>(
        &mut self,
        owner: &ReadyRecordAppender<S>,
    ) -> Result<(), Refusal> {
        self.append(record_append(owner, self.actual, self.remaining)?)
    }
    /// # Safety
    /// Same original owned recovered residence/identity obligations as
    /// `recovered_record_append`; no grant or Start is created by publication.
    pub unsafe fn recovered_record_append<S: NorStorage>(
        &mut self,
        identity: &AppendNativeIdentity,
        residence: &PreparedNorRecordResidence<S>,
    ) -> Result<(), Refusal> {
        // SAFETY: exact caller-owned pair and unchanged bounds/context forwarded.
        self.append(unsafe {
            recovered_record_append(identity, residence, self.actual, self.remaining)
        }?)
    }
}
#[cfg(test)]
#[path = "native_images_append_tests.rs"]
mod tests;
