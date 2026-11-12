//! Compact description of one physical append/read service residence.
//! The checked immutable capability image comes from native_service's ordinary
//! capability_offer; no decoded Kind, grant or second capacity is retained here.
use super::*;
use conduit_resource_snapshot::{
    append::PreparedNorRecordResidence,
    native_append::{AppendNativeIdentity, APPEND_CLASS, APPEND_MECHANISM},
    native_service::{ReadyRecordService, SERVICE_CAPABILITY},
    nor::NorStorage,
};
fn project_service(
    identity: &AppendNativeIdentity,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    let mut facts = facts!(identity, APPEND_CLASS, SERVICE_CAPABILITY);
    facts.mechanism = APPEND_MECHANISM;
    project(facts, actual, maximum_bytes)
}
pub fn record_service<S: NorStorage>(
    owner: &ReadyRecordService<S>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    if owner.remaining_slots().is_none() {
        return Err(Refusal::Context);
    }
    project_service(owner.native_identity(), actual, maximum_bytes)
}
/// # Safety
/// Root owns this exact recovered physical pair and will install the same pair
/// into ReadyRecordService after two original Issuer authorizations. Metadata
/// and recovery create neither operation authority nor an execution window.
pub unsafe fn recovered_record_service<S: NorStorage>(
    identity: &AppendNativeIdentity,
    residence: &PreparedNorRecordResidence<S>,
    actual: &c::HostAdvertisement,
    maximum_bytes: usize,
) -> Result<Projection, Refusal> {
    // SAFETY: forwards original recovered physical-pair obligations, with no IO.
    unsafe {
        crate::execution_authority::NativeFacts::recovered_record_service(identity, residence)
    }
    .map_err(|_| Refusal::Context)?;
    project_service(identity, actual, maximum_bytes)
}
impl Publication<'_> {
    pub fn record_service<S: NorStorage>(
        &mut self,
        owner: &ReadyRecordService<S>,
    ) -> Result<(), Refusal> {
        self.append(record_service(owner, self.actual, self.remaining)?)
    }
    /// # Safety
    /// Same original owned-pair obligations as recovered_record_service.
    pub unsafe fn recovered_record_service<S: NorStorage>(
        &mut self,
        identity: &AppendNativeIdentity,
        residence: &PreparedNorRecordResidence<S>,
    ) -> Result<(), Refusal> {
        // SAFETY: caller's exact pair and unchanged preparation bounds forwarded.
        self.append(unsafe {
            recovered_record_service(identity, residence, self.actual, self.remaining)
        }?)
    }
}
