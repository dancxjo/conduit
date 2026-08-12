#![no_std]

//! Exact bounded lifecycle for a Body born from checked Seed material.
//!
//! A Body is durable intent and obligations, never a physical host. A Wake is
//! one active maintenance interval; Lull ends that interval while preserving
//! the Body. Plans and Plays may be replaced within one Wake.

extern crate alloc;

mod candidate;
mod events;
mod hold;
mod identity;
mod lifecycle;
mod membership;
mod validation;

pub use candidate::*;
pub use events::{BodyLifecycleEvent, WakeLifecycleEvent};
pub use hold::*;
pub use identity::{
    BodyId, CandidateId, DiscoveryProofId, MembershipChangeId, MembershipProofId, PartId, SeedId,
    WakeId, MAX_LIFECYCLE_ID_BYTES,
};
pub use lifecycle::*;
pub use membership::*;
