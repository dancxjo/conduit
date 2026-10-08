//! Deterministic Rust bindings for checked native semantic Types.
//!
//! Generated Rust is a consumer of checked Conduit meaning. These helpers do
//! not derive semantic identity from Rust names, layout, or traits.

mod borrowed_contracts;
mod bounded;
pub use borrowed_contracts::validate_borrowed_native_contracts;
mod generate;
mod generate_conversion;
mod generate_direct;
mod generate_invariant;
#[cfg(test)]
mod generate_layout_tests;
mod generate_options;
pub use generate_options::MAXIMUM_GENERATED_NATIVE_FAMILY_TYPES;
mod generate_package;
mod generate_prepared_family;
#[cfg(test)]
mod generate_tests;
mod generate_value;
mod prepared_invariant_storage;
mod prepared_invariants;
pub use prepared_invariant_storage::{
    PreparedNativeInvariantStorageLimits, PreparedNativeInvariantStorageReceipt,
};
mod prepared_family;
mod prepared_family_contracts;
#[cfg(test)]
mod prepared_invariants_tests;
mod primitive;
pub use prepared_family::{
    NativeFamilyConversionProfile, NativeFamilyTypeDescriptor, PreparedNativeFamily,
    PreparedNativeFamilyLimits, PreparedNativeFamilyRefusal, PreparedNativeFamilyStorageReceipt,
    PreparedNativeRustBinding, MAXIMUM_NATIVE_FAMILY_TYPES,
};
pub use prepared_family_contracts::{
    NativeFamilyConstraintDescriptor, NativeFamilyContractDescriptor,
};
pub use prepared_invariants::{PreparedNativeInvariantAdmission, PreparedNativeInvariantRefusal};
mod value;

pub use bounded::{BoundedBytes, BoundedSequence, BoundedSequenceCapacityRefusal, BoundedText};
pub use conduit_core as semantic_core;
pub use generate::{
    generate_rust_bindings, generate_rust_bindings_with_external_bindings,
    generate_rust_bindings_with_forms, generate_rust_bindings_with_forms_and_external_bindings,
    ExternalNativeRustBinding, ExternalRustBindingGenerationError, RustBindingGenerationError,
    RustBindingModule, RustBindingOptions,
};
pub use generate_package::{
    generate_locked_package_rust_bindings, LockedPackageBindingSource,
    LockedPackageRustBindingInput, LockedRustBindingGenerationError,
};
pub use primitive::{primitive_from_structured, primitive_into_structured, NativePrimitive};
pub use value::{
    collection_element_type, nominal_representation_type, record_field_type, record_field_value,
    sequence_element_type, validate_native_contracts, validate_native_invariants,
    variant_payload_type, NativeBindingRefusal, NativeRustBinding,
};

/// A bounded refusal produced by any generated finite Form codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFormRefusal {
    WrongLength { actual: usize },
    InvalidTag { actual: u8 },
}

mod external_binding_validation;
