//! Std source half of the S4 toggle-demo distributed proof.
//!
//! Split by stable responsibility:
//! - `plan`: planning, advertisement resolution, two-fragment plan creation.
//! - `operation`: `ToggleSourceOperation` kernel state machine with mutation tests.
//! - `source`: `DistributedToggleSource` orchestration and WebSocket transport.

mod operation;
mod plan;
mod source;

pub use plan::{exact_distributed_toggle_plan, DistributedTogglePlan};
pub use source::{bind_listener, DistributedToggleSource};
