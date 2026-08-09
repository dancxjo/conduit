#![no_std]

//! Exact bounded lifecycle for a Body born from checked Seed material.
//!
//! A Body is durable intent and obligations, never a physical host. A Wake is
//! one active maintenance interval; Lull ends that interval while preserving
//! the Body. Plans and Plays may be replaced within one Wake.

extern crate alloc;

mod events;
mod identity;
mod lifecycle;

pub use events::{BodyLifecycleEvent, WakeLifecycleEvent};
pub use identity::{BodyId, SeedId, WakeId, MAX_LIFECYCLE_ID_BYTES};
pub use lifecycle::*;
