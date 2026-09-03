//! Bounded brownfield adaptation of one exact Tongues starter.

#[cfg(feature = "speech")]
mod contract;
#[cfg(feature = "speech")]
mod execution;
#[cfg(feature = "speech")]
mod pcm;
#[cfg(feature = "speech")]
mod planning;
#[cfg(feature = "speech")]
mod realization;
mod research_compute;
mod research_data;
mod research_form;
mod research_math;
mod research_model;
mod research_report;
#[cfg(feature = "speech")]
mod signs;
#[cfg(feature = "speech")]
mod specimen;

#[cfg(feature = "speech")]
pub use contract::*;
#[cfg(feature = "speech")]
pub use execution::*;
#[cfg(feature = "speech")]
pub use planning::*;
#[cfg(feature = "speech")]
pub use realization::*;
pub use research_compute::*;
pub use research_data::*;
pub use research_form::*;
pub use research_model::*;
pub use research_report::*;
#[cfg(feature = "speech")]
pub use signs::*;
#[cfg(feature = "speech")]
pub use specimen::*;
