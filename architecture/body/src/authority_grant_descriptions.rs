//! Structural grant descriptions are usable without authenticated transport.
//! These checks confer neither attribution nor native execution possession.
use crate::{MAX_CANDIDATE_CAPABILITIES, MAX_LIFECYCLE_ID_BYTES};
use conduit_core::{AuthorityGrant, HostAdvertisement};
use core::borrow::Borrow;
pub const MAX_MEMBER_RETURN_IMAGE_GRANTS: usize = MAX_CANDIDATE_CAPABILITIES;
pub const MAX_MEMBER_RETURN_IMAGE_GRANT_ID_BYTES: usize = 4096;
#[derive(Debug)]
pub(crate) struct Refusal;

/// Bounded structural checks only; attribution and freshness belong to admission.
pub(crate) fn validate<G: Borrow<AuthorityGrant>>(
    context: &HostAdvertisement,
    grants: &[G],
) -> Result<(), Refusal> {
    if grants.len() > MAX_MEMBER_RETURN_IMAGE_GRANTS {
        return Err(Refusal);
    }
    let mut total = 0usize;
    for (index, grant) in grants.iter().enumerate() {
        let grant = grant.borrow();
        if grant.host_id != context.host_id
            || grant.boot_id != context.boot_id
            || grants[..index].iter().any(|prior| {
                let prior = prior.borrow();
                prior.grant_id == grant.grant_id
                    || (prior.capability_id == grant.capability_id
                        && prior.contract_id == grant.contract_id
                        && prior.host_call_contract_id == grant.host_call_contract_id
                        && prior.subject_kind == grant.subject_kind)
            })
        {
            return Err(Refusal);
        }
        for id in [
            grant.grant_id.as_str(),
            grant.contract_id.as_str(),
            grant.host_call_contract_id.as_str(),
            grant.subject_kind.as_str(),
            grant.host_id.as_str(),
            grant.boot_id.as_str(),
            grant.capability_id.as_str(),
        ] {
            if id.is_empty() || id.len() > MAX_LIFECYCLE_ID_BYTES {
                return Err(Refusal);
            }
            total = total.checked_add(id.len()).ok_or(Refusal)?;
            if total > MAX_MEMBER_RETURN_IMAGE_GRANT_ID_BYTES {
                return Err(Refusal);
            }
        }
    }
    Ok(())
}
