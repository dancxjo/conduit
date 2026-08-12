//! Ordinary portable presentation execution on the ConduitOS display Base.

mod bool_play;
mod logic_multi_plan;
mod logic_multi_play;
#[cfg(test)]
mod logic_multi_tests;
mod logic_not_play;
mod math_clamp_play;
#[cfg(test)]
mod math_clamp_play_tests;
mod operation;
mod plan;
mod play;

pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const TEXT_SOURCE_KIND: &str = "conduitos/fixture-text-source";
pub use bool_play::{BoolPresentationError, BoolPresentationProof, prepare_bool, run_bool};
pub use logic_multi_plan::{PreparedLogicMulti, prepare_logic_multi};
pub use logic_multi_play::{LogicMultiError, LogicMultiProof, run_logic_multi};
pub use logic_not_play::{LogicNotError, LogicNotProof, prepare_not, run_not};
pub use math_clamp_play::{MathClampError, MathClampProof, prepare_clamp, run_clamp};
