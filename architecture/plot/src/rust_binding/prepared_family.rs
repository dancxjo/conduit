//! Finite, explicitly owned metadata for generated recursive Native conversion.
use super::prepared_family_contracts::{self, NativeFamilyContractDescriptor};
use super::{
    validate_borrowed_native_contracts, NativeBindingRefusal, NativeRustBinding,
    PreparedNativeInvariantAdmission, PreparedNativeInvariantRefusal,
    PreparedNativeInvariantStorageLimits,
};
use crate::NativeTypeValueContract;
use alloc::vec::Vec;
use conduit_core::{
    validate_canonical_structured_value, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape, ValidatedCanonicalStructuredValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    MAXIMUM_STRUCTURED_INFO_NODES,
};
use core::mem::size_of;

pub const MAXIMUM_NATIVE_FAMILY_TYPES: usize = 64;

/// Static external metadata comparison work, separate from owned heap quotas.
pub const MAXIMUM_NATIVE_FAMILY_EXTERNAL_METADATA_BYTES: usize =
    MAXIMUM_STRUCTURED_CANONICAL_BYTES * MAXIMUM_NATIVE_FAMILY_TYPES;

#[derive(Debug)]
pub struct NativeFamilyExternalEdge {
    pub descriptor: &'static NativeFamilyTypeDescriptor,
    /// Complete generated owning-crate metadata, independent of the Rust path
    /// that resolves `descriptor`. This shadow has no converter authority.
    pub expected: &'static NativeFamilyTypeDescriptor,
}

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
    pub external_edges: &'static [NativeFamilyExternalEdge],
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

pub(super) struct PreparedType {
    pub(super) descriptor: &'static NativeFamilyTypeDescriptor,
    contracts: Vec<NativeTypeValueContract>,
    laws: PreparedNativeInvariantAdmission,
    framing: Vec<u8>,
}

pub struct PreparedNativeFamily {
    pub(super) types: Vec<PreparedType>,
    pub(super) maximum_input_bytes: usize,
    pub(super) receipt: PreparedNativeFamilyStorageReceipt,
    pub(super) admission_domain:
        Option<super::prepared_family_admission::NativeAdmissionDomainOwner>,
}

/// Implemented by generated bindings; every named child uses this same owner.
/// Custom implementations must preserve complete child-before-parent Native
/// validation and independently account for their allocations. Scoped converters
/// may omit a node's validation only when its authenticated scope permits it.
/// This trait does not authenticate arbitrary custom implementation behavior;
/// capability issuance separately validates the complete canonical subtree.
pub trait PreparedNativeRustBinding: NativeRustBinding {
    const PREPARED_DESCRIPTOR: &'static NativeFamilyTypeDescriptor;
    fn from_borrowed_prepared(
        value: ValidatedCanonicalStructuredValue<'_>,
        family: &mut PreparedNativeFamily,
    ) -> Result<Self, NativeBindingRefusal>;
    /// Older imported bindings safely retain their complete validation path.
    fn from_borrowed_prepared_with_children(
        value: ValidatedCanonicalStructuredValue<'_>,
        family: &mut PreparedNativeFamily,
        _scope: &super::NativeChildAdmissionScope<'_>,
    ) -> Result<Self, NativeBindingRefusal> {
        Self::from_borrowed_prepared(value, family)
    }
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
        let mut external_metadata_budget = MAXIMUM_NATIVE_FAMILY_EXTERNAL_METADATA_BYTES;
        for root in roots {
            collect(
                root,
                &mut descriptors,
                &mut count,
                limits.maximum_types,
                &mut external_metadata_budget,
            )?;
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
            // A descriptor pointer cannot substitute a named child's original
            // complete Type. Charge the simultaneous parent + child decoder
            // peak before reconstructing any child for this check.
            for child in descriptor.children {
                let child_decode =
                    StructuredInfoType::canonical_decode_storage_bound(child.type_bytes)
                        .map_err(Refusal::InvalidType)?;
                let child_peak = retained
                    .checked_add(type_heap)
                    .and_then(|bytes| bytes.checked_add(child_decode))
                    .ok_or(Refusal::Capacity)?;
                admit(child_peak, limits.maximum_preparation_peak_bytes)?;
                peak = peak.max(child_peak);
                let child_type = StructuredInfoType::from_canonical_bytes(child.type_bytes)
                    .map_err(Refusal::InvalidType)?;
                if !contains_child_type(&value_type, &child_type) {
                    return Err(Refusal::ConflictingDescriptor);
                }
            }
            let contract_heap = prepared_family_contracts::storage_bound(descriptor.contracts)
                .ok_or(Refusal::Capacity)?;
            let framing_heap = if descriptor.laws.is_empty() {
                0
            } else {
                limits.maximum_input_bytes
            };
            let metadata_heap = contract_heap
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
            admission_domain: None,
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

    /// Exact descriptor readiness, inspected before a consuming target call.
    /// Equal encoded Type bytes do not substitute for the prepared descriptor.
    pub fn contains_descriptor(&self, descriptor: &'static NativeFamilyTypeDescriptor) -> bool {
        self.types
            .iter()
            .any(|ty| core::ptr::eq(ty.descriptor, descriptor))
    }

    pub fn check_type(
        &self,
        descriptor: &'static NativeFamilyTypeDescriptor,
        value: ValidatedCanonicalStructuredValue<'_>,
    ) -> Result<(), NativeBindingRefusal> {
        if value.type_bytes() != descriptor.type_bytes || !self.contains_descriptor(descriptor) {
            return Err(wrong_type());
        }
        Ok(())
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
        validate_borrowed_native_contracts(value, &ty.contracts)?;
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

fn contains_child_type(parent: &StructuredInfoType, expected: &StructuredInfoType) -> bool {
    let contains =
        |child: &StructuredInfoType| child == expected || contains_child_type(child, expected);
    match parent.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => contains(representation),
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => contains(element),
        StructuredInfoTypeShape::Record { fields, .. } => {
            fields.iter().any(|field| contains(field.value_type()))
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            cases.iter().any(|case| contains(case.payload_type()))
        }
        StructuredInfoTypeShape::Leaf(_) => false,
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
    external_metadata_budget: &mut usize,
) -> Result<(), PreparedNativeFamilyRefusal> {
    if descriptor.children.len() > MAXIMUM_NATIVE_FAMILY_TYPES
        || descriptor.type_bytes.len() > MAXIMUM_STRUCTURED_CANONICAL_BYTES
    {
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
    super::prepared_family_external::validate_edges(descriptor, external_metadata_budget)?;
    if *count >= maximum {
        return Err(PreparedNativeFamilyRefusal::Capacity);
    }
    descriptors[*count] = Some(descriptor);
    *count += 1;
    for child in descriptor.children {
        collect(child, descriptors, count, maximum, external_metadata_budget)?;
    }
    Ok(())
}
