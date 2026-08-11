//! Ordinary portable presentation execution on the ConduitOS display Base.

mod operation;
mod plan;
mod play;

pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const TEXT_SOURCE_KIND: &str = "conduitos.fixture/text-source";
