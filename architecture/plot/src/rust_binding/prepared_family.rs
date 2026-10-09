//! Finite, explicitly owned metadata for generated recursive Native conversion.
use super::prepared_family_contracts::{self, NativeFamilyContractDescriptor};
use super::{
    NativeBindingRefusal, NativeRustBinding, PreparedNativeInvariantAdmission,
    PreparedNativeInvariantRefusal, PreparedNativeInvariantStorageLimits,
};
use crate::NativeTypeValueContract;
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, StructuredInfoRefusal, StructuredInfoType,
    ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES,
};
use core::mem::size_of;

pub const MAXIMUM_NATIVE_FAMILY_TYPES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeFamilyConversionProfile {
    Record,
    Nominal,
    Variant,
}

#[derive(Debug)]
pub struct NativeFamilyTypeDescriptor {
    pub type_bytes: &'static [u8],
    pub laws: &'static [&'static [u8]],
    pub contracts: &'static [NativeFamilyContractDescriptor],
    pub children: &'static [&'static NativeFamilyTypeDescriptor],
    pub conversion_profile: NativeFamilyConversionProfile,
    /// Generated Rust inline layout, used solely to bound conversion allocations.
    pub maximum_inline_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeFamilyLimits {
    pub maximum_types: usize,
    pub maximum_laws_per_type: usize,
    pub maximum_input_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub maximum_conversion_requested_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreparedNativeFamilyStorageReceipt {
    pub types: usize,
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub conversion_requested_bytes_bound: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreparedNativeFamilyRefusal {
    Capacity,
    ConflictingDescriptor,
    UnsupportedConversionProfile,
    InvalidType(StructuredInfoRefusal),
    InvalidContract(conduit_core::ConstraintDefinitionError),
    InvalidLaws(PreparedNativeInvariantRefusal),
}

struct PreparedType {
    descriptor: &'static NativeFamilyTypeDescriptor,
    contracts: Vec<NativeTypeValueContract>,
    laws: PreparedNativeInvariantAdmission,
    framing: Vec<u8>,
    record_access: Option<conduit_core::PreparedCanonicalRecordAccess<'static>>,
}

pub struct PreparedNativeFamily {
    types: Vec<PreparedType>,
    maximum_input_bytes: usize,
    receipt: PreparedNativeFamilyStorageReceipt,
}

/// Implemented by generated bindings; every named child uses this same owner.
/// Custom implementations must independently account for their allocations.
pub trait PreparedNativeRustBinding: NativeRustBinding {
    const PREPARED_DESCRIPTOR: &'static NativeFamilyTypeDescriptor;
    fn from_borrowed_prepared(
        value: ValidatedCanonicalStructuredValue<'_>,
        family: &mut PreparedNativeFamily,
    ) -> Result<Self, NativeBindingRefusal>;
}

impl PreparedNativeFamily {
    pub fn prepare(
        roots: &[&'static NativeFamilyTypeDescriptor],
        limits: PreparedNativeFamilyLimits,
    ) -> Result<Self, PreparedNativeFamilyRefusal> {
        use PreparedNativeFamilyRefusal as Refusal;
        if roots.len() > 16
            || limits.maximum_types > MAXIMUM_NATIVE_FAMILY_TYPES
            || limits.maximum_input_bytes > MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err(Refusal::Capacity);
        }
        let mut descriptors = [None; MAXIMUM_NATIVE_FAMILY_TYPES];
        let mut count = 0;
        for root in roots {
            collect(root, &mut descriptors, &mut count, limits.maximum_types)?;
        }
        let mut retained = count
            .checked_mul(size_of::<PreparedType>())
            .ok_or(Refusal::Capacity)?;
        admit(retained, limits.maximum_retained_bytes)?;
        admit(retained, limits.maximum_preparation_peak_bytes)?;
        let mut peak = retained;
        let mut maximum_inline = 0;
        let mut longest_path = 0;
        let mut types = Vec::with_capacity(count);
        for descriptor in descriptors[..count].iter().flatten() {
            if descriptor.conversion_profile != NativeFamilyConversionProfile::Record
                && !descriptor.laws.is_empty()
            {
                return Err(Refusal::UnsupportedConversionProfile);
            }
            let decode = StructuredInfoType::canonical_decode_storage_bound(descriptor.type_bytes)
                .map_err(Refusal::InvalidType)?;
            let decode_peak = retained.checked_add(decode).ok_or(Refusal::Capacity)?;
            admit(decode_peak, limits.maximum_preparation_peak_bytes)?;
            peak = peak.max(decode_peak);
            let value_type = StructuredInfoType::from_canonical_bytes(descriptor.type_bytes)
                .map_err(Refusal::InvalidType)?;
            let type_heap = value_type.owned_heap_bytes();
            let contract_heap = prepared_family_contracts::storage_bound(descriptor.contracts)
                .ok_or(Refusal::Capacity)?;
            let framing_heap = if descriptor.laws.is_empty() {
                0
            } else {
                limits.maximum_input_bytes
            };
            let record_heap = if descriptor.type_bytes.first() == Some(&2) {
                conduit_core::PreparedCanonicalRecordAccess::storage_bound(descriptor.type_bytes)
                    .map_err(Refusal::InvalidType)?
            } else {
                0
            };
            let metadata_heap = contract_heap
                .checked_add(record_heap)
                .ok_or(Refusal::Capacity)?
                .checked_add(framing_heap)
                .ok_or(Refusal::Capacity)?;
            let base = retained
                .checked_add(metadata_heap)
                .ok_or(Refusal::Capacity)?;
            admit(base, limits.maximum_retained_bytes)?;
            let metadata_peak = base.checked_add(type_heap).ok_or(Refusal::Capacity)?;
            admit(metadata_peak, limits.maximum_preparation_peak_bytes)?;
            peak = peak.max(metadata_peak);
            let contracts = prepared_family_contracts::materialize(descriptor.contracts)
                .map_err(Refusal::InvalidContract)?;
            let framing = Vec::with_capacity(framing_heap);
            let record_access = if descriptor.type_bytes.first() == Some(&2) {
                Some(
                    conduit_core::PreparedCanonicalRecordAccess::prepare(
                        descriptor.type_bytes,
                        record_heap,
                    )
                    .map_err(Refusal::InvalidType)?,
                )
            } else {
                None
            };
            let (laws, law_receipt) =
                PreparedNativeInvariantAdmission::from_canonical_laws_with_storage_limits(
                    &value_type,
                    descriptor.laws,
                    PreparedNativeInvariantStorageLimits {
                        maximum_laws: limits.maximum_laws_per_type,
                        maximum_input_bytes: limits.maximum_input_bytes,
                        maximum_retained_bytes: limits
                            .maximum_retained_bytes
                            .checked_sub(base)
                            .ok_or(Refusal::Capacity)?,
                        maximum_preparation_peak_bytes: limits
                            .maximum_preparation_peak_bytes
                            .checked_sub(base)
                            .ok_or(Refusal::Capacity)?,
                    },
                )
                .map_err(Refusal::InvalidLaws)?;
            peak = peak.max(
                base.checked_add(law_receipt.preparation_peak_heap_bytes_bound)
                    .ok_or(Refusal::Capacity)?,
            );
            retained = base
                .checked_add(law_receipt.retained_heap_bytes_bound)
                .ok_or(Refusal::Capacity)?;
            maximum_inline = maximum_inline.max(descriptor.maximum_inline_bytes);
            for contract in descriptor.contracts {
                longest_path = longest_path.max(contract.representation_path.len());
            }
            drop(value_type);
            types.push(PreparedType {
                descriptor,
                contracts,
                laws,
                framing,
                record_access,
            });
        }
        let conversion = maximum_inline
            .checked_mul(MAXIMUM_STRUCTURED_INFO_NODES)
            // Cumulative geometric Vec allocation requests, including minimum
            // capacity and moved old buffers, are conservatively below 4 slots
            // per canonical node across generated containers and Box payloads.
            .and_then(|bytes| bytes.checked_mul(4))
            .and_then(|bytes| bytes.checked_add(limits.maximum_input_bytes))
            .and_then(|bytes| bytes.checked_add(longest_path))
            .ok_or(Refusal::Capacity)?;
        admit(conversion, limits.maximum_conversion_requested_bytes)?;
        Ok(Self {
            types,
            maximum_input_bytes: limits.maximum_input_bytes,
            receipt: PreparedNativeFamilyStorageReceipt {
                types: count,
                retained_heap_bytes_bound: retained,
                preparation_peak_heap_bytes_bound: peak,
                conversion_requested_bytes_bound: conversion,
            },
        })
    }

    pub const fn storage_receipt(&self) -> PreparedNativeFamilyStorageReceipt {
        self.receipt
    }

    pub fn decode<T: PreparedNativeRustBinding>(
        &mut self,
        canonical: &[u8],
    ) -> Result<T, NativeBindingRefusal> {
        if canonical.len() > self.maximum_input_bytes {
            return Err(wrong_type());
        }
        let value = validate_canonical_structured_value(canonical)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        self.check_type(T::PREPARED_DESCRIPTOR, value)?;
        T::from_borrowed_prepared(value, self)
    }

    pub fn check_type(
        &self,
        descriptor: &'static NativeFamilyTypeDescriptor,
        value: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<(), NativeBindingRefusal> {
        if value.type_bytes() != descriptor.type_bytes
            || !self
                .types
                .iter()
                .any(|ty| core::ptr::eq(ty.descriptor, descriptor))
        {
            return Err(wrong_type());
        }
        Ok(())
    }

    /// Exact prepared record framing; complete descriptor identity remains required.
    pub fn record_field<'a>(
        &self,
        descriptor: &'static NativeFamilyTypeDescriptor,
        value: ValidatedCanonicalStructuredValue<'a>,
        name: &str,
    ) -> Result<Option<ValidatedCanonicalStructuredValue<'a>>, NativeBindingRefusal> {
        self.check_type(descriptor, value)?;
        let ty = self
            .types
            .iter()
            .find(|ty| core::ptr::eq(ty.descriptor, descriptor))
            .ok_or_else(wrong_type)?;
        ty.record_access
            .as_ref()
            .ok_or_else(wrong_type)?
            .field(value, name)
            .map_err(NativeBindingRefusal::InvalidValue)
    }

    /// Generated converters call after their children in the original constructor order.
    pub fn validate(
        &mut self,
        descriptor: &'static NativeFamilyTypeDescriptor,
        value: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<(), NativeBindingRefusal> {
        self.check_type(descriptor, value)?;
        let ty = self
            .types
            .iter_mut()
            .find(|ty| core::ptr::eq(ty.descriptor, descriptor))
            .ok_or_else(wrong_type)?;
        // The legacy variant from_structured entrance decodes its children and
        // directly constructs the case, without invoking the public case constructor.
        if descriptor.conversion_profile == NativeFamilyConversionProfile::Variant {
            return Ok(());
        }
        super::borrowed_contracts::validate_borrowed_native_contracts_with_record_access(
            value,
            &ty.contracts,
            ty.record_access.as_ref(),
        )?;
        if descriptor.laws.is_empty() {
            return Ok(());
        }
        let length = descriptor
            .type_bytes
            .len()
            .checked_add(value.value_node().len())
            .ok_or_else(wrong_type)?;
        if length > ty.framing.capacity() {
            return Err(wrong_type());
        }
        ty.framing.clear();
        ty.framing.extend_from_slice(descriptor.type_bytes);
        ty.framing.extend_from_slice(value.value_node());
        // Fresh full validation of newly framed bytes occurs inside the bank.
        ty.laws.validate(&ty.framing)
    }
}

fn admit(bytes: usize, maximum: usize) -> Result<(), PreparedNativeFamilyRefusal> {
    if bytes > maximum {
        Err(PreparedNativeFamilyRefusal::Capacity)
    } else {
        Ok(())
    }
}
fn wrong_type() -> NativeBindingRefusal {
    NativeBindingRefusal::InvalidValue(StructuredInfoRefusal::WrongType)
}

fn collect(
    descriptor: &'static NativeFamilyTypeDescriptor,
    descriptors: &mut [Option<&'static NativeFamilyTypeDescriptor>; MAXIMUM_NATIVE_FAMILY_TYPES],
    count: &mut usize,
    maximum: usize,
) -> Result<(), PreparedNativeFamilyRefusal> {
    if descriptor.children.len() > MAXIMUM_NATIVE_FAMILY_TYPES {
        return Err(PreparedNativeFamilyRefusal::Capacity);
    }
    for prior in descriptors[..*count].iter().flatten() {
        if core::ptr::eq(*prior, descriptor) {
            return Ok(());
        }
        if prior.type_bytes == descriptor.type_bytes {
            return Err(PreparedNativeFamilyRefusal::ConflictingDescriptor);
        }
    }
    if *count >= maximum {
        return Err(PreparedNativeFamilyRefusal::Capacity);
    }
    descriptors[*count] = Some(descriptor);
    *count += 1;
    for child in descriptor.children {
        collect(child, descriptors, count, maximum)?;
    }
    Ok(())
}
