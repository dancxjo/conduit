#![no_std]
//! Body workspace orchestration shared by browser and native Hosts.
//!
//! The body lifecycle owns identity and transitions. Hosts plan and realize work;
//! this product retains their exact lifecycle evidence and foreground selection.
extern crate alloc;

mod continuity;
mod flow;
pub mod invitation;
pub mod library;
mod lifecycle;
pub mod tutorial;
pub mod tutorial_presenter;
pub use lifecycle::{WorkspaceBody, WorkspaceBodyError, WorkspaceRealization};
