#![no_std]
//! Body workspace orchestration shared by browser and native Hosts.
//!
//! The Body lifecycle owns identity and transitions. Hosts plan and realize work;
//! this product retains their exact lifecycle evidence and foreground selection.
extern crate alloc;

mod continuity;
mod current_hosts;
mod flow;
pub mod invitation;
pub mod library;
mod lifecycle;
pub mod tutorial;
pub use current_hosts::{CurrentHostOfferError, CurrentHostOffers};
pub use lifecycle::{WorkspaceBody, WorkspaceBodyError, WorkspaceRealization};
