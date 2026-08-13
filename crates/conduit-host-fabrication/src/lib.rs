//! Versioned, bounded Host PROFILE composition.
//!
//! Validation describes machinery that BUILD may fabricate. It deliberately
//! cannot create a runtime Host, Boot, offer, Body, Plan, or Play.

mod canonical;
mod catalog;
mod model;
mod validation;

pub use canonical::{canonical_profile_json, ProfileId};
pub use catalog::{FabricationCatalog, PrerequisiteNode};
pub use model::*;
pub use validation::{validate_profile, ProfileDiagnostic, ValidatedHostProfile};

#[cfg(test)]
mod tests;
