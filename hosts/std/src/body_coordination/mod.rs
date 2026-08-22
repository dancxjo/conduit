//! Bounded two-Host coordination through the production kernel and planned Lines.

mod conformance;
mod endpoint;
mod line;
mod receipt;

pub use conformance::run_in_process;
pub use endpoint::{CoordinationEndpoint, CoordinationOffer};
pub use line::{run_forebrain, run_motherbrain, CoordinationLineError};
pub use receipt::{BodyCoordinationReceipt, CoordinationFailure, CoordinationRole};

#[cfg(test)]
mod tests;
