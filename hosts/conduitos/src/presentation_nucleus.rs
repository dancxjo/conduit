//! Ordinary portable presentation execution on the ConduitOS display Base.

mod bool_play;
mod operation;
mod plan;
mod play;

pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const TEXT_SOURCE_KIND: &str = "conduitos/fixture-text-source";
pub use bool_play::{BoolPresentationError, BoolPresentationProof, prepare_bool, run_bool};
