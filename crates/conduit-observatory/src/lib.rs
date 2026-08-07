#![no_std]

extern crate alloc;

mod model;
mod projection;
mod render;

pub use model::*;
pub use projection::{build_report, unsupported_state, validate_snapshot, SNAPSHOT_SCHEMA};
pub use render::render_text_report;

#[cfg(test)]
mod tests;
