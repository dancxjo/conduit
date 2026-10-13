extern crate alloc;
extern crate self as conduit_semantic_catalog;
#[path="/home/dancxjo/conduit-4898-common-greeting/semantics/catalog/src/flow_zip.rs"]
mod catalog_zip;
pub use catalog_zip::*;
#[path="/home/dancxjo/conduit-4898-common-greeting/semantics/catalog/src/flow_zip_finite.rs"]
mod catalog_finite;
pub use catalog_finite::*;
#[path="/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/fargan/public-model-driver/bounded-zip/flow_zip.rs"]
pub mod flow_zip;
#[cfg(test)]
#[path="/home/dancxjo/conduit-4898-common-greeting/architecture/plot/tests/prepared_structured_payload/allocation.rs"]
mod test_allocations;
#[cfg(test)]
#[path="/home/dancxjo/Documents/Codex/2026-10-06/goal-please-finish-and-close-4907/work/fargan/public-model-driver/bounded-zip/tests.rs"]
mod bounded_tests;
