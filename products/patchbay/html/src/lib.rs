//! Bounded delivery adapter for the read-only HTML Patchbay renderer.
//!
//! Its JSON carries the portable Presentation and exact Manifestation result;
//! DOM/SVG objects and HTTP mechanics remain renderer-local transport facts.

mod body_workbench;
mod body_workbench_fixture;
mod body_workbench_inventory;
mod cross_host;
mod demo;
mod form_sources;
mod front_door;
mod learned_demo;
mod server;
mod snapshot;
#[path = "server/theme.rs"]
mod theme;
mod transport_types;

pub use body_workbench::{
    attach_body_workbench, body_workbench_snapshot, body_workbench_snapshot_with_forms,
    BodyWorkbenchError,
};
pub use body_workbench_fixture::{body_workbench_fixture_forms, body_workbench_fixture_snapshot};
pub use cross_host::{cross_host_demonstration_snapshot, CrossHostRendererError};
pub use demo::{
    demonstration_snapshot, llm_documentary_snapshot, llm_embodiment_snapshot,
    recursive_form_demonstration_snapshot, text_lab_split_loss_snapshot, text_lab_split_snapshot,
};
pub use form_sources::{
    load_form_sources, FormSource, FormSourceError, MAX_ADDITIONAL_FORMS, MAX_FORM_LABEL_BYTES,
};
pub use front_door::front_door_snapshot;
pub use learned_demo::learned_demonstration_snapshot;
pub use server::{PatchbayHtmlServer, ServerError, MAX_HTTP_REQUEST_BYTES, MAX_THEME_CSS_BYTES};
pub use snapshot::{SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA};
pub use transport_types::*;

pub fn application_theme_css() -> Vec<u8> {
    theme::render_theme_css(&patchbay_model::CONDUIT_APPLICATION_THEME)
}
