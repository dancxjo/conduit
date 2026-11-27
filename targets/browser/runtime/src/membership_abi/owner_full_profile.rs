//! Optional full Mask Host profile: concrete interfaces and a separately funded
//! Boot witness must exist before its capabilities can leave this WASM instance.
use super::*;
use crate::owner_face_mask::full_profile_pool;
use conduit_browser_mask_offer::full_profile as profile;
const ERROR_PROFILE: i32 = -266;
thread_local! {static FROZEN:RefCell<bool>=const{RefCell::new(false)};}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    host_id: HostId,
    boot_id: BootId,
    profile: String,
}

pub(super) fn freeze() -> Result<(), String> {
    if full_profile_pool::interface_ready() {
        full_profile_pool::freeze()?;
    }
    FROZEN.with(|f| *f.borrow_mut() = true);
    Ok(())
}
pub(super) fn reset() -> Result<(), String> {
    // A failed reset must retain the incarnation and its accounting; swallowing
    // live-custody pressure would leave prepared outputs without their budget.
    full_profile_pool::can_reset()?;
    crate::owner_face_mask::reset_full_interface()?;
    full_profile_pool::reset()?;
    FROZEN.with(|f| *f.borrow_mut() = false);
    Ok(())
}
pub(super) fn capture_advertisement(
    host: &conduit_core::HostAdvertisement,
    bytes: &[u8],
) -> Result<(), String> {
    if full_profile_pool::ready(&host.host_id, &host.boot_id) {
        full_profile_pool::capture_advertisement(host, bytes)?;
    }
    Ok(())
}
pub(super) fn extend(host: &mut conduit_core::HostAdvertisement) -> Result<(), String> {
    if !full_profile_pool::ready(&host.host_id, &host.boot_id) {
        return Ok(());
    }
    let offer = profile::offer();
    if let Some(existing) = host
        .capabilities
        .iter()
        .find(|c| c.capability_id == offer.capability_id)
    {
        if existing != &offer {
            return Err("Mask Full offer alias".into());
        }
    } else {
        host.capabilities.push(offer);
    }
    let pool = conduit_core::resource_offer(
        profile::MEMORY_POOL,
        profile::MEMORY_CLASS,
        profile::PREPARATION_BYTES * profile::PREPARATION_SLOTS as u32,
    );
    if let Some(existing) = host.resources.iter().find(|r| r.pool_id == pool.pool_id) {
        if existing != &pool {
            return Err("Mask Full pool alias".into());
        }
    } else {
        host.resources.push(pool);
    }
    host.capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    host.resources.sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    Ok(())
}
#[no_mangle]
pub extern "C" fn conduit_browser_membership_owner_full_profile_admit(length: u32) -> i32 {
    clear_output();
    if length == 0 || length as usize > INPUT_CAPACITY || FROZEN.with(|f| *f.borrow()) {
        return ERROR_PROFILE;
    }
    let request: Admission =
        match INPUT.with(|i| serde_json::from_slice(&i.borrow()[..length as usize])) {
            Ok(v) => v,
            Err(_) => return ERROR_PROFILE,
        };
    IDENTITY.with(|i| {
        let i = i.borrow();
        let Some(i) = i.as_ref() else {
            return ERROR_NOT_INITIALIZED;
        };
        if request.profile != profile::PROFILE
            || &request.host_id != i.host_id()
            || &request.boot_id != i.boot_id()
        {
            return ERROR_PROFILE;
        }
        if super::owner_component::invalidate_snapshot().is_err() {
            return ERROR_PROFILE;
        }
        if full_profile_pool::prepare(&request.host_id, &request.boot_id).is_err() {
            return ERROR_PROFILE;
        }
        match crate::owner_face_mask::prepare_full_interface() {
            Ok(()) => STATUS_READY,
            Err(_) => {
                // Preserve the budget if existing manifestation custody prevents
                // interface cleanup; a later ordinary clear/reset can retire it.
                let _ = reset();
                ERROR_PROFILE
            }
        }
    })
}
