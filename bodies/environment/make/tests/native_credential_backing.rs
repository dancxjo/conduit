//! Actual private-profile custody through fallible owner backing. This does not
//! qualify the ESP32 Root workspace, SDK configuration, or physical execution.
extern crate alloc;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_credential_backing.rs"]
mod backing;
#[allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_known_credentials.rs"]
mod known;
use conduit_net::{KnownSecurity, PrivateNetworkInput};

fn selected_owner() -> (known::Owner, known::Selection) {
    let mut owner = known::Owner::empty(2);
    let key = owner
        .upsert_after_acknowledgement(
            PrivateNetworkInput::new(
                b"fixture",
                b"public-fixture-key",
                KnownSecurity::Wpa2Personal,
                0,
            )
            .unwrap(),
            |_| Ok(1),
        )
        .unwrap();
    let selection = owner.freeze(key).unwrap();
    (owner, selection)
}

#[test]
fn original_profile_revision_and_frozen_selection_survive_unique_backing() {
    let (owner, selection) = selected_owner();
    let owner = backing::try_box(owner).unwrap();
    assert_eq!(owner.store_revision(), 1);
    assert_eq!(owner.accepted_updates(), 1);
    assert_eq!(owner.selection(), Some(selection));
}

#[test]
fn null_refuses_actual_selected_private_owner_without_recreating_it() {
    let (owner, _) = selected_owner();
    // SAFETY: null is permitted by the actual allocator seam. The untransferred
    // Owner drops its profile inputs and frozen Secret through their own wipes.
    let refused = unsafe { backing::try_box_with(owner, |_| core::ptr::null_mut()) };
    assert!(refused.is_err());
}
