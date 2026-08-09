//! Std source half of the S4 toggle-demo distributed proof.
//!
//! Split by stable responsibility:
//! - `plan`: planning, advertisement resolution, two-fragment plan creation.
//! - `operation`: `ToggleSourceOperation` kernel state machine with mutation tests.
//! - `source`: `DistributedToggleSource` struct, preparation, and host-op adapter.
//! - `line`: WebSocket session and line transport for the source.

mod line;
mod operation;
mod plan;
mod source;

pub use line::bind_listener;
pub use plan::{exact_distributed_toggle_plan, DistributedTogglePlan};
pub use source::DistributedToggleSource;
