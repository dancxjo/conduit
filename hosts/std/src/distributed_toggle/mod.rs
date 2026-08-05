//! Std source half of the S4 toggle-demo distributed proof.
//!
//! Split by stable responsibility:
//! - `plan`: planning, advertisement resolution, two-fragment plan creation.
//! - `operation`: `ToggleSourceOperation` kernel state machine with mutation tests.
//! - `source`: `DistributedToggleSource` struct, preparation, and host-op adapter.
//! - `carrier`: WebSocket session and carrier transport for the source.

mod carrier;
mod operation;
mod plan;
mod source;

pub use carrier::bind_listener;
pub use plan::{exact_distributed_toggle_plan, DistributedTogglePlan};
pub use source::DistributedToggleSource;
