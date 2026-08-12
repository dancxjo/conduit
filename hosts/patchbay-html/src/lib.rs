//! Bounded delivery adapter for the read-only HTML Patchbay renderer.
//!
//! Its JSON carries the portable Presentation and exact Manifestation result;
//! DOM/SVG objects and HTTP mechanics remain renderer-local transport facts.

mod cross_host;
mod demo;
mod front_door;
mod server;
mod snapshot;
mod transport_types;

pub use cross_host::{cross_host_demonstration_snapshot, CrossHostRendererError};
pub use demo::demonstration_snapshot;
pub use front_door::front_door_snapshot;
pub use server::{PatchbayHtmlServer, ServerError, MAX_HTTP_REQUEST_BYTES, MAX_THEME_CSS_BYTES};
pub use snapshot::{SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA};
pub use transport_types::*;
