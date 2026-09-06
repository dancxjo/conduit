#![cfg_attr(not(test), no_std)]

//! Host-neutral HTTP and JSON semantic contracts.
//!
//! This crate owns portable value shape, validation, codecs, Kind identity,
//! and canonical Form catalog installation. It owns no Host implementation,
//! execution profile, host operation, resource, authority, or transport.

extern crate alloc;

mod contract;
pub mod http;
pub use contract::PortableKindContract;
pub use http::*;
mod json;
pub use json::*;
mod json_collection;
pub use json_collection::*;
mod json_value;
pub use json_value::*;
