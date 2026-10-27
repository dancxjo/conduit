//! Trusted installed Make expression material; never a received-program proof.
//! Export, wire reading, validation and mutable-state construction stay separate.
mod export;
mod import;
mod validate;
mod wire;

pub use export::MakeExpressionMaterial;
pub(super) use wire::{next_member, scalar_instruction, structured_instruction};
