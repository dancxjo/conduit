//! Deterministic Rust bindings for checked native semantic Types.
//!
//! Generated Rust is a consumer of checked Conduit meaning. These helpers do
//! not derive semantic identity from Rust names, layout, or traits.

mod bounded;
mod generate;
mod generate_conversion;
mod generate_invariant;
mod generate_package;
#[cfg(test)]
mod generate_tests;
mod generate_value;
mod primitive;
mod value;

pub use bounded::{BoundedBytes, BoundedSequence, BoundedSequenceCapacityRefusal, BoundedText};
pub use conduit_core as semantic_core;
pub use generate::{
    generate_rust_bindings, generate_rust_bindings_with_codes, RustBindingGenerationError,
    RustBindingModule, RustBindingOptions,
};
pub use generate_package::{
    generate_locked_package_rust_bindings, ExternalNativeRustBinding, LockedPackageBindingSource,
    LockedPackageRustBindingInput, LockedRustBindingGenerationError,
};
pub use primitive::{primitive_from_structured, primitive_into_structured, NativePrimitive};
pub use value::{
    collection_element_type, nominal_representation_type, record_field_type, record_field_value,
    sequence_element_type, validate_native_contracts, validate_native_invariants,
    variant_payload_type, NativeBindingRefusal, NativeRustBinding,
};

/// A bounded refusal produced by any generated finite code codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeCodeRefusal {
    WrongLength { actual: usize },
    InvalidTag { actual: u8 },
}
