#![no_std]

extern crate alloc;

mod causal_explanation;
mod clock_quality;
mod cost_comparison_explanation;
mod cost_explanation;
mod cost_plan_explanation;
pub use cost_plan_explanation::{render_cost_planned_realizations, render_cost_planning_refusal};
mod cost_policy_explanation;
mod evidence_lineage;
mod execution_artifact;
mod model;
mod pool_cost_explanation;
pub use pool_cost_explanation::render_pool_cost_selection;
mod projection;
mod render;
mod sound;
mod usefulness;
mod validation;

pub use causal_explanation::*;
pub use clock_quality::*;
pub use cost_explanation::*;
pub use evidence_lineage::*;
pub use execution_artifact::*;
pub use model::*;
pub use projection::{build_report, unsupported_state, SNAPSHOT_SCHEMA};
pub use render::render_text_report;
pub use sound::*;
pub use usefulness::*;
pub use validation::validate_snapshot;

#[cfg(test)]
mod causal_explanation_tests;
#[cfg(test)]
mod clock_quality_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod usefulness_tests;

#[cfg(test)]
mod cost_explanation_tests;

#[cfg(test)]
mod cost_hard_refusal_tests;
#[cfg(test)]
mod cost_work_estimate_tests;

#[cfg(test)]
mod cost_plan_explanation_tests;

#[cfg(test)]
mod pool_cost_explanation_tests;

mod global_placement_explanation;
pub use global_placement_explanation::render_global_placement_comparison;

#[cfg(test)]
mod global_placement_explanation_tests;
