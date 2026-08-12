//! Ordinary portable presentation execution on the ConduitOS display Base.

mod bool_play;
mod logic_not_play;
mod operation;
mod plan;
mod play;

pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const TEXT_SOURCE_KIND: &str = "conduitos/fixture-text-source";
pub use bool_play::{BoolPresentationError, BoolPresentationProof, prepare_bool, run_bool};
pub use logic_not_play::{LogicNotError, LogicNotProof, prepare_not, run_not};
