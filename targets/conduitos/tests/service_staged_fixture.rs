//! Host-only definitions for the exact shared proof descriptor/fixed formatter.
//! Passing these alone never establishes actual CPL3 or staged IPC execution.
use conduitos::sign_format;
#[path = "../domain/service_bulk.rs"]
mod service_bulk;
#[path = "../src/ordinary_domain_proof/service_memory/staged/format.rs"]
mod staged_format;
