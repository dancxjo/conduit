//! Checked HID Source admission against caller-supplied Host offers and grants.
//! Preparation performs no device effects and issues no Base possession.
use crate::protocol_source::{
    PreparedProtocolArtifact, PreparedProtocolEntry, usb_hid_endpoint_package,
    usb_hid_keyboard_order_package,
};
use alloc::{collections::BTreeMap, string::String};
use conduit_core::{AuthorityGrant, BaseImplementationId, HostAdvertisement};
use conduit_planner::PlanningOptions;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HidSourceRole {
    Keyboard,
    /// Eight calls and ordered class state; Root supplies the admitted window.
    KeyboardCapture,
    Mouse,
}

impl HidSourceRole {
    fn entry(self) -> &'static str {
        match self {
            Self::KeyboardCapture => "usb-hid-keyboard-capture-window",
            Self::Keyboard => "usb-hid-keyboard-batch-endpoint",
            Self::Mouse => "usb-hid-mouse-endpoint",
        }
    }
}

/// The caller supplies the current initialized endpoint Back and its explicit
/// authority grants. A checked Plan is not executable possession: the native
/// Root must separately bind its actual retained DMA and issued Base handle.
pub fn prepare(
    role: HidSourceRole,
    host: &HostAdvertisement,
    authority_grants: &[AuthorityGrant],
) -> Result<PreparedProtocolArtifact, String> {
    let package = match role {
        HidSourceRole::KeyboardCapture => usb_hid_keyboard_order_package(),
        _ => usb_hid_endpoint_package(),
    }
    .map_err(|_| "usb-hid-source-package")?;
    let bytes = serde_json::to_vec(&package).map_err(|_| "usb-hid-source-package")?;
    let entry = PreparedProtocolEntry::prepare(&bytes, role.entry())
        .map_err(|error| alloc::format!("usb-hid-source-check: {error:?}"))?;
    let mut host = host.clone();
    entry
        .publish_pure_backs(&mut host)
        .map_err(|_| "usb-hid-source-pure-backs")?;
    let hosts = [host];
    let placements = entry
        .placements(&hosts)
        .map_err(|error| alloc::format!("usb-hid-source-placement: {error:?}"))?;
    entry
        .plan(
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
        )
        .map_err(|error| alloc::format!("usb-hid-source-admission: {error:?}"))
}

#[cfg(test)]
#[path = "hid_source_plan_tests.rs"]
mod tests;
