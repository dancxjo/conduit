#![no_std]

extern crate alloc;

mod model;
mod projection;
mod render;
mod sound;
mod usefulness;
mod validation;

pub use model::*;
pub use projection::{build_report, unsupported_state, SNAPSHOT_SCHEMA};
pub use render::render_text_report;
pub use sound::*;
pub use usefulness::*;
pub use validation::validate_snapshot;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod usefulness_tests;
