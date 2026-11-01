//! One optional component admission frozen for the initialized Host incarnation.
use super::*;
use conduit_browser_mask_offer::owner_component::{
    admits_owner_component, owner_component_descriptor, owner_component_offer,
    OwnerComponentAdmission,
};
use conduit_core::HostAdvertisement;

const ERROR_COMPONENT: i32 = -265;
#[derive(Default)]
struct Profile {
    admitted: bool,
    exported: bool,
}
thread_local! { static PROFILE: RefCell<Profile> = RefCell::new(Profile::default()); }

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Admission {
    host_id: HostId,
    boot_id: BootId,
    component: OwnerComponentAdmission,
}

pub(super) fn reset() {
    PROFILE.with(|profile| *profile.borrow_mut() = Profile::default());
}
pub(super) fn freeze() {
    PROFILE.with(|profile| profile.borrow_mut().exported = true);
}

pub(crate) fn current_advertisement(
    host: HostId,
    boot: BootId,
) -> Result<HostAdvertisement, String> {
    IDENTITY.with(|identity| {
        let identity = identity.borrow();
        let current = identity
            .as_ref()
            .ok_or("browser membership is not initialized")?;
        if current.host_id() != &host || current.boot_id() != &boot {
            return Err("browser component profile belongs to another Host incarnation".into());
        }
        let mut advertisement = crate::installed_browser::membership_advertisement(host, boot);
        advertisement
            .capabilities
            .retain(|offer| offer.kind_id.as_str() != conduit_face::COMPONENT_MASK_KIND);
        if PROFILE.with(|profile| profile.borrow().admitted) {
            advertisement.capabilities.push(owner_component_offer()?);
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        Ok(advertisement)
    })
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_owner_component_descriptor() -> i32 {
    clear_output();
    if IDENTITY.with(|identity| identity.borrow().is_none()) {
        return ERROR_NOT_INITIALIZED;
    }
    let bytes = match owner_component_descriptor()
        .and_then(|value| serde_json::to_vec(&value).map_err(|error| error.to_string()))
    {
        Ok(bytes) if bytes.len() <= OUTPUT_CAPACITY => bytes,
        _ => return ERROR_COMPONENT,
    };
    OUTPUT.with(|output| output.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
    OUTPUT_LEN.with(|length| *length.borrow_mut() = bytes.len());
    STATUS_READY
}

#[no_mangle]
pub extern "C" fn conduit_browser_membership_owner_component_admit(length: u32) -> i32 {
    clear_output();
    if length == 0 || length as usize > INPUT_CAPACITY {
        return ERROR_INPUT;
    }
    let request: Admission =
        match INPUT.with(|input| serde_json::from_slice(&input.borrow()[..length as usize])) {
            Ok(request) => request,
            Err(_) => return ERROR_COMPONENT,
        };
    IDENTITY.with(|identity| {
        let identity = identity.borrow();
        let Some(identity) = identity.as_ref() else {
            return ERROR_NOT_INITIALIZED;
        };
        if request.host_id != *identity.host_id()
            || request.boot_id != *identity.boot_id()
            || !admits_owner_component(&request.component)
        {
            return ERROR_COMPONENT;
        }
        PROFILE.with(|profile| {
            let mut profile = profile.borrow_mut();
            if profile.exported || profile.admitted {
                return ERROR_COMPONENT;
            }
            profile.admitted = true;
            STATUS_READY
        })
    })
}

#[cfg(test)]
mod tests;
