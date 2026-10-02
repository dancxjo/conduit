//! Plot-checking catalog for the Signal contracts.
//!
//! This is current semantic data. It is deliberately separate from the legacy
//! hosted implementation registry in `host_profile`.

use super::{pulse_semantic_contract, show_semantic_contract};
use conduit_plot::ProfileCatalog;

pub fn signal_profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    catalog
        .insert_kind(pulse_semantic_contract())
        .expect("signal profile kinds are unique");
    catalog
        .insert_kind(show_semantic_contract())
        .expect("signal profile kinds are unique");
    catalog
}
