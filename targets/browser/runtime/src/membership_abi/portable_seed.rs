//! Human-carried portable admission. Receipt correlation is not owner authentication.
//! One attempt per initialized Boot prevents canceled invitations being silently reused.
use super::*;
use conduit_body::{
    MembershipCredential, PortableAdmissionReceipt, PortableInvitation,
    PortableSpawnAdmissionRequest, SPAWN_ADMISSION_REQUEST_SCHEMA,
};

const REFUSED: i32 = -270;
struct Pending {
    request: PortableSpawnAdmissionRequest,
    expires_at: u64,
}
#[derive(Default)]
struct State {
    attempted: bool,
    pending: Option<Pending>,
    credential: Option<MembershipCredential>,
}
thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}
pub(super) fn reset() {
    STATE.with(|state| *state.borrow_mut() = State::default());
}

// Reuse the existing finite membership output buffer for larger receipt input;
// no additional permanent multi-megabyte input arena is needed.
#[no_mangle]
pub extern "C" fn conduit_browser_seed_input_ptr() -> *mut u8 {
    OUTPUT.with(|output| output.borrow_mut().as_mut_ptr())
}
#[no_mangle]
pub extern "C" fn conduit_browser_seed_input_capacity() -> u32 {
    OUTPUT_CAPACITY as u32
}
fn read<T: serde::de::DeserializeOwned>(length: u32) -> Result<T, ()> {
    let length = length as usize;
    if length == 0 || length > OUTPUT_CAPACITY {
        return Err(());
    }
    OUTPUT.with(|output| serde_json::from_slice(&output.borrow()[..length]).map_err(|_| ()))
}
fn output(value: &impl serde::Serialize) -> i32 {
    match serde_json::to_vec(value) {
        Ok(bytes) if bytes.len() <= OUTPUT_CAPACITY => {
            clear_output();
            OUTPUT.with(|output| output.borrow_mut()[..bytes.len()].copy_from_slice(&bytes));
            OUTPUT_LEN.with(|length| *length.borrow_mut() = bytes.len());
            STATUS_READY
        }
        _ => REFUSED,
    }
}
fn now(low: u32, high: u32) -> u64 {
    u64::from(low) | (u64::from(high) << 32)
}
fn current() -> Result<conduit_core::HostAdvertisement, ()> {
    IDENTITY.with(|identity| {
        let identity = identity.borrow();
        let identity = identity.as_ref().ok_or(())?;
        owner_component::current_advertisement(
            identity.host_id().clone(),
            identity.boot_id().clone(),
        )
        .map_err(|_| ())
    })
}

/// Explicit caller authorization elects to sign; it is not membership authority.
#[no_mangle]
pub extern "C" fn conduit_browser_seed_prepare(
    length: u32,
    low: u32,
    high: u32,
    authorized: u32,
) -> i32 {
    let invitation = read::<PortableInvitation>(length);
    clear_output();
    if authorized != 1 {
        return REFUSED;
    }
    let Ok(mut invitation) = invitation else {
        return REFUSED;
    };
    let result = (|| {
        invitation.validate(now(low, high)).map_err(|_| ())?;
        let advertisement = current()?;
        let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).map_err(|_| ())?;
        let signature = secret.sign(&invitation.claim.signing_transcript(
            &advertisement.host_id,
            &advertisement.boot_id,
            advertisement.offer_generation,
        ));
        let request = PortableSpawnAdmissionRequest {
            schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: invitation.claim.invitation_id.clone(),
            body_id: invitation.claim.body_id.clone(),
            host_advertisement: advertisement,
            nonce: invitation.claim.nonce,
            signature: signature.to_vec(),
            membership_admitted: false,
            plan_created: false,
            play_created: false,
        };
        request.validate().map_err(|_| ())?;
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            if state.attempted {
                return Err(());
            }
            if output(&request) != STATUS_READY {
                return Err(());
            }
            state.attempted = true;
            state.pending = Some(Pending {
                request,
                expires_at: invitation.claim.expires_at_millis,
            });
            owner_component::freeze();
            Ok(())
        })
    })();
    invitation.secret.fill(0);
    if result.is_ok() {
        STATUS_READY
    } else {
        REFUSED
    }
}

/// The HUMAN authenticates the owner/carrier before authorizing this import.
/// Existing receipt v1 proves no owner signature and contains no invitation ID.
#[no_mangle]
pub extern "C" fn conduit_browser_seed_complete(
    length: u32,
    low: u32,
    high: u32,
    authorized: u32,
) -> i32 {
    let receipt = read::<PortableAdmissionReceipt>(length);
    clear_output();
    if authorized != 1 {
        return REFUSED;
    }
    let Ok(receipt) = receipt else {
        return REFUSED;
    };
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        let Some(pending) = &state.pending else {
            return REFUSED;
        };
        if now(low, high) >= pending.expires_at
            || current().as_ref() != Ok(&pending.request.host_advertisement)
            || receipt.validate_against(&pending.request).is_err()
        {
            return REFUSED;
        }
        let result = output(&receipt.credential);
        if result == STATUS_READY {
            state.credential = Some(receipt.credential);
            state.pending = None;
        }
        result
    })
}

/// Retained membership is historical evidence; this creates no current presence.
#[no_mangle]
pub extern "C" fn conduit_browser_seed_credential() -> i32 {
    clear_output();
    STATE.with(|state| match &state.borrow().credential {
        Some(credential) => output(credential),
        None => REFUSED,
    })
}
#[no_mangle]
pub extern "C" fn conduit_browser_seed_cancel() -> i32 {
    clear_output();
    STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.attempted = true;
        state.pending = None;
    });
    STATUS_READY
}

#[cfg(test)]
mod tests;
