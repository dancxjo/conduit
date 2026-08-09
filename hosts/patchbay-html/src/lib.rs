//! Bounded delivery adapter for the read-only HTML Patchbay renderer.
//!
//! Its JSON carries the portable Presentation and exact Manifestation result;
//! DOM/SVG objects and HTTP mechanics remain renderer-local transport facts.

mod demo;
mod server;
mod snapshot;
mod transport_types;

pub use demo::demonstration_snapshot;
pub use server::{PatchbayHtmlServer, ServerError, MAX_HTTP_REQUEST_BYTES};
pub use snapshot::{SnapshotError, MAX_SNAPSHOT_BYTES, SNAPSHOT_SCHEMA};
pub use transport_types::*;
