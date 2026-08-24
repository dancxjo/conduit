//! Versioned, bounded Host PROFILE composition.
//!
//! Validation describes machinery that BUILD may fabricate. It deliberately
//! cannot create a runtime Host, Boot, offer, Body, Plan, or Play.

mod architecture_package;
mod body_description;
mod build;
mod canonical;
mod catalog;
mod configuration;
mod configuration_descriptors;
mod esp32;
mod esp32_wroom32;
mod model;
mod runtime;
mod spore;
mod validation;

pub use architecture_package::*;
pub use body_description::*;
pub use build::*;
pub use canonical::{canonical_profile_json, ProfileId};
pub use catalog::{FabricationCatalog, PrerequisiteNode};
pub use configuration::*;
pub use configuration_descriptors::*;
pub use esp32::*;
pub use esp32_wroom32::*;
pub use model::*;
pub use runtime::*;
pub use spore::*;
pub use validation::{validate_profile, ProfileDiagnostic, ValidatedHostProfile};

#[cfg(test)]
mod body_building_tests;
#[cfg(test)]
mod configuration_tests;
#[cfg(test)]
mod esp32_tests;
#[cfg(test)]
mod esp32_wroom32_tests;
#[cfg(test)]
mod native_presenter_tests;
#[cfg(test)]
mod tests;
