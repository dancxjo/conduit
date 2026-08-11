//! Ordinary portable presentation execution on the ConduitOS display Base.

mod operation;
mod plan;
mod play;

pub use plan::{FORM_SOURCE, PreparedPresentationPlay, prepare};
pub use play::{PresentationProof, PresentationRunError, run};

pub const DISPLAY_KIND: &str = "conduitos.fixture/framebuffer-present";
pub const LAYOUT_SINK_KIND: &str = "conduitos.fixture/layout-observe";
pub const TEXT_SOURCE_KIND: &str = "conduitos.fixture/text-source";
pub const DISPLAY_HOST_OPERATION: &str = "conduitos.host/framebuffer-present@1";
