//! Authored body construction, checked multi-host composition, and Spore binding.
//!
//! This layer depends on Host make to produce PROFILE, BUILD, and IMAGE
//! artifacts. It does not create a current host, Boot, membership, Plan, or Play.

mod biography_projection;
mod body_description;
mod body_source;
mod construction_source;
mod current_frame;
mod evidence_attachment;
mod planning;
mod readable_history;
mod spore;
mod workload_session;

pub use biography_projection::*;
pub use body_description::*;
pub use body_source::*;
pub use construction_source::*;
pub use current_frame::*;
pub use evidence_attachment::*;
pub use planning::*;
pub use readable_history::*;
pub use spore::*;
pub use workload_session::*;

#[cfg(test)]
mod body_building_tests;
#[cfg(test)]
mod planning_session_tests;
