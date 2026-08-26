//! Versioned, bounded Host PROFILE composition.
//!
//! Validation describes machinery that BUILD may fabricate. It deliberately
//! cannot create a runtime Host, Boot, offer, Body, Plan, or Play.

mod body_description;
mod body_source;
mod build;
mod canonical;
mod catalog;
mod configuration;
mod configuration_descriptors;
mod construction_source;
mod model;
mod package_contract;
mod runtime;
mod spore;
mod validation;

#[cfg(test)]
mod test_packages;

pub use body_description::*;
pub use body_source::*;
pub use build::*;
pub use canonical::{canonical_profile_json, ProfileId};
pub use catalog::FabricationCatalog;
pub use configuration::*;
pub use configuration_descriptors::*;
pub use construction_source::*;
pub use model::*;
pub use package_contract::*;
pub use runtime::*;
pub use spore::*;
pub use validation::{validate_profile, ProfileDiagnostic, ValidatedHostProfile};

#[cfg(test)]
mod body_building_tests;
#[cfg(test)]
mod configuration_tests;
#[cfg(test)]
mod native_presenter_tests;
#[cfg(test)]
mod tests;
