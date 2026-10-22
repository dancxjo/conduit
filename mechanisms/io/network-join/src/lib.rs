#![no_std]
//! Selected network Host Calls. No radio initialization or scheduling policy.
extern crate alloc;
mod contract;
pub mod installation;
mod lease;
mod owner;
pub use contract::{AttachmentRecord, NetworkJoinProvider, PreparedJoinContract};
pub use owner::{AssignedJoinSelection, JoinCallRefusal, NetworkJoinHostCall};
