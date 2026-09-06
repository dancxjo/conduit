#![no_std]

//! Exact bounded lifecycle for a Body born with an initial checked Form workset.
//!
//! A Body is durable intent and obligations, never a physical host. A Wake is
//! one active maintenance interval; Lull ends that interval while preserving
//! the Body. Plans and Plays may be replaced within one Wake.

extern crate alloc;

#[cfg(feature = "authenticated-admission")]
mod admission;
mod biography;
mod candidate;
mod continuity;
mod events;
mod hold;
mod identity;
mod legacy;
mod lifecycle;
mod membership;
mod offers;
#[cfg(feature = "authenticated-admission")]
mod pico_admission;
mod presence;
mod provenance;
mod reservations;
mod space;
mod validation;
mod workload_plan;
mod workset;

#[cfg(feature = "authenticated-admission")]
pub use admission::*;
pub use biography::*;
pub use candidate::*;
pub use continuity::*;
pub use events::{BodyLifecycleEvent, WakeLifecycleEvent};
pub use hold::*;
#[cfg(feature = "authenticated-admission")]
pub use identity::{AdmissionId, MembershipCredentialId, SpawnInvitationId};
pub use identity::{
    BodyId, CandidateId, DiscoveryProofId, MembershipChangeId, MembershipProofId, PartId, WakeId,
    MAX_LIFECYCLE_ID_BYTES,
};
pub use legacy::*;
pub use lifecycle::*;
pub use membership::*;
pub use offers::*;
#[cfg(feature = "authenticated-admission")]
pub use pico_admission::*;
pub use presence::*;
pub use provenance::*;
pub use reservations::*;
pub use space::*;
pub use workload_plan::*;
pub use workset::*;
