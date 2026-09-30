//! Deterministic Rust bindings for checked native semantic Types.
//!
//! Generated Rust is a consumer of checked Conduit meaning. These helpers do
//! not derive semantic identity from Rust names, layout, or traits.

mod bounded;
mod generate;
mod generate_conversion;
mod generate_package;
#[cfg(test)]
mod generate_tests;
mod generate_value;
mod primitive;
mod value;

pub use bounded::{BoundedBytes, BoundedSequence, BoundedText};
pub use generate::{
    generate_rust_bindings, RustBindingGenerationError, RustBindingModule, RustBindingOptions,
};
pub use generate_package::{
    generate_locked_package_rust_bindings, LockedPackageBindingSource,
    LockedPackageRustBindingInput, LockedRustBindingGenerationError,
};
pub use primitive::{primitive_from_structured, primitive_into_structured, NativePrimitive};
pub use value::{
    nominal_representation_type, record_field_type, record_field_value, sequence_element_type,
    validate_native_contracts, variant_payload_type, NativeBindingRefusal, NativeRustBinding,
};
