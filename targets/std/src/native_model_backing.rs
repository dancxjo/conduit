//! Exact hosted CPU native-memory admission; ordinary model Gear integration is separate.
mod ledger;
mod profile;
mod protocol;
mod supervisor;
mod worker;
pub use ledger::{BackingLedger, BackingRole, BackingSpan, SharedPoolUse};
pub use profile::{NativeBackingGrant, NativeBackingRefusal, NativeCpuProfile};
pub use supervisor::PreparedNativeModel;
pub use worker::worker_main;
