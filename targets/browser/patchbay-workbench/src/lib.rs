//! Bounded delivery adapter for the read-only HTML Patchbay renderer.
//!
//! Its JSON carries the portable Presentation and exact Manifestation result;
//! DOM/SVG objects and HTTP mechanics remain renderer-local transport facts.

pub mod application_resources;
mod body_workbench;
mod body_workbench_fixture;
mod body_workbench_inventory;
mod cross_host;
mod demo;
mod front_door;
mod learned_demo;
mod plot_sources;
mod server;
mod snapshot;
#[path = "server/theme.rs"]
mod theme;
mod transport_types;

pub use body_workbench::{
    attach_body_workbench, body_workbench_snapshot, body_workbench_snapshot_with_plots,
    BodyWorkbenchError,
};
pub use body_workbench_fixture::{body_workbench_fixture_plots, body_workbench_fixture_snapshot};
pub use cross_host::{cross_host_demonstration_snapshot, CrossHostRendererError};
pub use demo::{
    demonstration_snapshot, llm_documentary_snapshot, llm_embodiment_snapshot,
    recursive_plot_demonstration_snapshot, text_lab_split_loss_snapshot, text_lab_split_snapshot,
};
pub use front_door::{front_door_snapshot, one_plot_two_fronts_snapshot};
pub use learned_demo::learned_demonstration_snapshot;
pub use plot_sources::{
    load_plot_sources, PlotSource, PlotSourceError, MAX_ADDITIONAL_PLOTS, MAX_PLOT_LABEL_BYTES,
};
pub use server::{PatchbayHtmlServer, ServerError, MAX_HTTP_REQUEST_BYTES, MAX_THEME_CSS_BYTES};
pub use snapshot::{SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA};
pub use transport_types::*;

pub fn application_theme_css() -> Vec<u8> {
    theme::render_theme_css(&conduit_presentation::CONDUIT_APPLICATION_THEME)
}
