//! Authored Body construction, checked multi-Host composition, and Spore binding.
//!
//! This layer depends on Host fabrication to produce PROFILE, BUILD, and IMAGE
//! artifacts. It does not create a current Host, Boot, membership, Plan, or Play.

mod body_description;
mod body_source;
mod construction_source;
mod spore;

pub use body_description::*;
pub use body_source::*;
pub use construction_source::*;
pub use spore::*;

#[cfg(test)]
mod body_building_tests;
