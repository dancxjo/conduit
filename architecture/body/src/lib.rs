#![no_std]

//! Exact bounded lifecycle for a Body born with an initial checked Form workset.
//!
//! A Body is durable intent and obligations, never a physical host. A Wake is
//! one active maintenance interval; Lull ends that interval while preserving
//! the Body. Plans and Plays may be replaced within one Wake.
//!
//! Capacity taxonomy:
//! - Form, Part, Line, active Wake/Plan, and simultaneous resource limits are
//!   working-set bounds and may refuse additional concurrent work.
//! - Body Signs, membership events, retained Wakes, and biography records are
//!   active-history bounds; exact prefixes cross a checkpoint into bounded,
//!   digest-linked archive segments instead of ending the Body's lifetime.
//! - monotonic revisions/sequences and bounded identity encodings are protocol
//!   bounds; exhaustion or malformed identity remains a permanent refusal.

extern crate alloc;

mod administration;
#[cfg(feature = "authenticated-admission")]
mod admission;
mod biography;
mod candidate;
mod character_purpose;
mod character_purpose_continuity;
mod continuity;
mod conversation;
mod durable_body;
mod emergency_control;
mod emergency_key;
mod events;
#[cfg(feature = "authenticated-admission")]
mod federation;
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
mod rendezvous;
mod rendezvous_attempt;
mod rendezvous_cbor;
mod rendezvous_manifestation;
mod rendezvous_validation;
mod reservations;
mod space;
mod startup;
mod validation;
mod workload_plan;
mod workload_transition;
mod workset;

pub use administration::*;
#[cfg(feature = "authenticated-admission")]
pub use admission::*;
pub use biography::*;
pub use candidate::*;
pub use character_purpose::*;
pub use character_purpose_continuity::*;
pub use continuity::*;
pub use conversation::*;
pub use durable_body::*;
pub use emergency_control::*;
pub use emergency_key::*;
pub use events::{BodyLifecycleEvent, WakeLifecycleEvent};
#[cfg(feature = "authenticated-admission")]
pub use federation::*;
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
pub use rendezvous::*;
pub use rendezvous_attempt::*;
pub use rendezvous_cbor::*;
pub use rendezvous_manifestation::*;
pub use reservations::*;
pub use space::*;
pub use startup::*;
pub use workload_plan::*;
pub use workload_transition::*;
pub use workset::*;
