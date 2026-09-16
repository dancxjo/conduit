//! Versioned, bounded Host PROFILE composition.
//!
//! Validation describes machinery that BUILD may fabricate. It deliberately
//! cannot create a runtime Host, Boot, offer, Body, Plan, or Play.

mod artifact_output;
mod build;
mod canonical;
mod catalog;
mod configuration;
mod configuration_descriptors;
mod construction_source;
mod deployment_carrier;
mod deployment_download;
mod deployment_network_boot;
mod deployment_removable;
mod deployment_uf2;
mod deployment_vm;
mod model;
mod package_contract;
mod release_catalog;
mod runtime;
mod validation;

#[cfg(test)]
mod test_packages;

pub use artifact_output::*;
pub use build::*;
pub use canonical::{canonical_profile_json, ProfileId};
pub use catalog::FabricationCatalog;
pub use configuration::*;
pub use configuration_descriptors::*;
pub use construction_source::*;
pub use deployment_carrier::*;
pub use deployment_download::*;
pub use deployment_network_boot::*;
pub use deployment_removable::*;
pub use deployment_uf2::*;
pub use deployment_vm::*;
pub use model::*;
pub use package_contract::*;
pub use release_catalog::*;
pub use runtime::*;
pub use validation::{validate_profile, ProfileDiagnostic, ValidatedHostProfile};

#[cfg(test)]
mod configuration_tests;
#[cfg(test)]
mod native_presenter_tests;
#[cfg(test)]
mod tests;
