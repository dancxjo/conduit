#![no_std]

extern crate alloc;

mod model;
mod projection;
mod render;
mod validation;

pub use model::*;
pub use projection::{build_report, unsupported_state, SNAPSHOT_SCHEMA};
pub use render::render_text_report;
pub use validation::validate_snapshot;

#[cfg(test)]
mod tests;
