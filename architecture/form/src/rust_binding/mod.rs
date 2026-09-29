//! Deterministic Rust bindings for checked native semantic Types.
//!
//! Generated Rust is a consumer of checked Conduit meaning. These helpers do
//! not derive semantic identity from Rust names, layout, or traits.

mod bounded;
mod value;

pub use bounded::{BoundedBytes, BoundedSequence, BoundedText};
pub use value::{NativeBindingRefusal, NativeRustBinding};
