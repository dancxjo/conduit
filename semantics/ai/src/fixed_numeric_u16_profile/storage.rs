//! Separate value-contract-only preparation capability. Invariant-bearing Source
//! profiles stay valid but refuse this entrance; their legacy laws are unchanged.
use super::*;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct U16ContractOnlyStorageReceipt {
    pub preparation_requested_bytes_bound: usize,
    pub retained_heap_bytes_bound: usize,
    pub retained_accounted_heap_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum U16ContractOnlyPreparationRefusal {
    Capacity,
    Validation,
    UnsupportedInvariantPreparation,
}
fn add(a: usize, b: usize) -> Result<usize, U16ContractOnlyPreparationRefusal> {
    a.checked_add(b)
        .ok_or(U16ContractOnlyPreparationRefusal::Capacity)
}
// Existing StructuredInfoValue::finish calls canonical_bytes: the exact Type
// prefix Vec is followed by one leaf tag, four length bytes, and two scalar bytes.
// For these U16 Types prefix>=7; its first push grows capacity to2*prefix, which
// holds the complete node. Charge BOTH original and replacement allocations.
fn finish_requests(prefix: usize) -> Result<usize, U16ContractOnlyPreparationRefusal> {
    if prefix < 7 {
        return Err(U16ContractOnlyPreparationRefusal::Validation);
    }
    add(
        prefix,
        prefix
            .checked_mul(2)
            .ok_or(U16ContractOnlyPreparationRefusal::Capacity)?,
    )
}
impl U16ProfileBack {
    pub fn storage_reservation_contract_only(
        profile: &PreparedU16Profile,
    ) -> Result<U16ContractOnlyStorageReceipt, U16ContractOnlyPreparationRefusal> {
        if !profile.checked.invariants.is_empty() {
            return Err(U16ContractOnlyPreparationRefusal::UnsupportedInvariantPreparation);
        }
        let StructuredInfoTypeShape::Nominal { representation, .. } =
            profile.checked.value_type.shape()
        else {
            return Err(U16ContractOnlyPreparationRefusal::Validation);
        };
        if !matches!(representation.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/u16")
        {
            return Err(U16ContractOnlyPreparationRefusal::Validation);
        }
        let prefix = profile
            .checked
            .value_type
            .canonical_byte_length()
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        let representation_prefix = representation
            .canonical_byte_length()
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        let maximum = add(prefix, 7)?;
        if maximum != profile.maximum_bytes as usize {
            return Err(U16ContractOnlyPreparationRefusal::Validation);
        }
        let validator = PreparedStructuredValueValidator::storage_reservation(
            &profile.checked.value_type,
            maximum,
        )
        .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        let array = profile
            .checked
            .value_contracts
            .len()
            .checked_mul(core::mem::size_of::<CheckedValueContract>())
            .ok_or(U16ContractOnlyPreparationRefusal::Capacity)?;
        let mut preparation = add(
            add(
                representation.owned_heap_bytes(),
                profile.checked.value_type.owned_heap_bytes(),
            )?,
            2,
        )?;
        preparation = add(preparation, finish_requests(representation_prefix)?)?;
        preparation = add(preparation, finish_requests(prefix)?)?;
        preparation = add(preparation, maximum)?; // exact final canonical_bytes_with_limit
        preparation = add(preparation, validator.preparation_requested_bytes_bound)?;
        preparation = add(preparation, array)?;
        let mut retained = add(add(maximum, validator.retained_heap_bytes_bound)?, array)?;
        for contract in &profile.checked.value_contracts {
            let r = contract
                .contract
                .clone_storage_reservation()
                .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
            preparation = add(preparation, r.preparation_requested_bytes_bound)?;
            retained = add(retained, r.retained_heap_bytes_bound)?;
        }
        Ok(U16ContractOnlyStorageReceipt {
            preparation_requested_bytes_bound: preparation,
            retained_heap_bytes_bound: retained,
            retained_accounted_heap_bytes: 0,
        })
    }
    /// All complete requests are checked before cloning original Types/contracts.
    /// Zero is merely the original private output template, never Source admission;
    /// the unchanged Step path checks every retained value contract before output.
    pub fn prepare_contract_only_with_storage_limits(
        profile: &PreparedU16Profile,
        flow: bool,
        maximum_preparation_requested_bytes: usize,
        maximum_retained_heap_bytes: usize,
    ) -> Result<(Self, U16ContractOnlyStorageReceipt), U16ContractOnlyPreparationRefusal> {
        let mut r = Self::storage_reservation_contract_only(profile)?;
        if r.preparation_requested_bytes_bound > maximum_preparation_requested_bytes
            || r.retained_heap_bytes_bound > maximum_retained_heap_bytes
        {
            return Err(U16ContractOnlyPreparationRefusal::Capacity);
        }
        let StructuredInfoTypeShape::Nominal { representation, .. } =
            profile.checked.value_type.shape()
        else {
            return Err(U16ContractOnlyPreparationRefusal::Validation);
        };
        let primitive = StructuredInfoValue::leaf(representation.clone(), vec![0, 0])
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        let output = StructuredInfoValue::nominal(profile.checked.value_type.clone(), primitive)
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?
            .canonical_bytes_with_limit(profile.maximum_bytes as usize)
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        if output.len() != profile.maximum_bytes as usize {
            return Err(U16ContractOnlyPreparationRefusal::Validation);
        }
        let scalar_offset = output
            .len()
            .checked_sub(2)
            .ok_or(U16ContractOnlyPreparationRefusal::Validation)?;
        let validator =
            PreparedStructuredValueValidator::new(&profile.checked.value_type, output.len())
                .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        validator
            .validate(&output)
            .map_err(|_| U16ContractOnlyPreparationRefusal::Validation)?;
        let mut contracts = Vec::with_capacity(profile.checked.value_contracts.len());
        contracts.extend(
            profile
                .checked
                .value_contracts
                .iter()
                .map(|c| c.contract.clone()),
        );
        let back = Self {
            output,
            scalar_offset,
            validator,
            contracts,
            invariants: Vec::new(),
            flow,
            staged: false,
            finished: false,
            cancelled: false,
            committed_frames: 0,
        };
        r.retained_accounted_heap_bytes = back.local_accounted_heap_bytes();
        Ok((back, r))
    }
}
