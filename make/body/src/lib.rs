//! Authored body construction, checked multi-host composition, and Spore binding.
//!
//! This layer depends on Host make to produce PROFILE, BUILD, and IMAGE
//! artifacts. It does not create a current host, Boot, membership, Plan, or Play.

mod biography_projection;
mod body_description;
mod body_source;
mod construction_source;
mod current_frame;
mod deployment_draft;
mod deployment_encounter;
pub mod deployment_runtime;
pub mod deployment_runtime_catalog;
mod evidence_attachment;
mod host_configuration_encounter;
mod joining_host;
mod joining_host_production;
mod joining_host_release;
mod planning;
mod prepared_birth;
mod readable_history;
mod spore;
mod workbench_face;
mod workbench_inventory;
mod workload_session;

pub use biography_projection::*;
pub use body_description::*;
pub use body_source::*;
pub use construction_source::*;
pub use current_frame::*;
pub use deployment_draft::*;
pub use deployment_encounter::*;
pub use evidence_attachment::*;
pub use host_configuration_encounter::*;
pub use joining_host::*;
pub use joining_host_production::*;
pub use joining_host_release::*;
pub use planning::*;
pub use prepared_birth::*;
pub use readable_history::*;
pub use spore::*;
pub use workbench_face::*;
pub use workbench_inventory::*;
pub use workload_session::*;

#[cfg(test)]
mod body_building_tests;
#[cfg(test)]
mod planning_session_tests;
