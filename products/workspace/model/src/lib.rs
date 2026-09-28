#![no_std]
//! Body workspace orchestration shared by browser and native Hosts.
//!
//! The body lifecycle owns identity and transitions. Hosts plan and realize work;
//! this product retains their exact lifecycle evidence and foreground selection.
extern crate alloc;

pub mod invitation;
pub mod library;
