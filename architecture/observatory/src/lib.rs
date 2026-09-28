#![no_std]

extern crate alloc;

mod causal_explanation;
mod evidence_lineage;
mod execution_artifact;
mod model;
mod projection;
mod render;
mod sound;
mod usefulness;
mod validation;

pub use causal_explanation::*;
pub use evidence_lineage::*;
pub use execution_artifact::*;
pub use model::*;
pub use projection::{build_report, unsupported_state, SNAPSHOT_SCHEMA};
pub use render::render_text_report;
pub use sound::*;
pub use usefulness::*;
pub use validation::validate_snapshot;

#[cfg(test)]
mod causal_explanation_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod usefulness_tests;
