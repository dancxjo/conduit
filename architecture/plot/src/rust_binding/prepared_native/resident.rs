//! Resident validation data; Make owns native catalogue and witness selection.
use super::*;
use conduit_core::{
    BorrowedFixedIntegerContract, BorrowedStructuredValueValidator, FixedIntegerContractSpec,
};
mod generate;
pub use generate::{generate_resident_native_validator, ResidentNativeGenerationRefusal};

#[derive(Clone, Copy, Debug)]
pub enum StaticNativePathStep {
    Representation,
    Field(&'static str),
    Each,
    Variant(&'static str),
}
/// Immutable checked path and predicate emitted by Make, never incoming authority.
pub struct StaticNativeContract {
    pub scope: u16,
    pub contract: u16,
    pub path: &'static [StaticNativePathStep],
    pub predicate: &'static FixedIntegerContractSpec<'static>,
}
/// No owned Type, contract, path, canonical input or scratch buffers.
/// This checks values only; it creates no ordinary Native admission witness,
/// offer or authority. Root must bind the checked image and admit complete state,
/// stack, schema traversal and predicate work before installing this realization.
// Cloning copies validated immutable views only: no value, mutable equipment,
// reservation or Native admission witness is duplicated. Constructor children
// can reuse the parent's checked definition without preparing it repeatedly.
#[derive(Clone)]
pub struct ResidentNativeValidator<const CONTRACTS: usize> {
    schema: BorrowedStructuredValueValidator<'static>,
    contracts: &'static [StaticNativeContract; CONTRACTS],
    scopes: &'static [&'static [StaticNativePathStep]],
    predicates: [Option<BorrowedFixedIntegerContract<'static>>; CONTRACTS],
    maximum_occurrences: usize,
    intrinsic_empty: bool,
}
impl<const N: usize> ResidentNativeValidator<N> {
    pub fn new(
        schema: &'static [u8],
        maximum_bytes: usize,
        contracts: &'static [StaticNativeContract; N],
        scopes: &'static [&'static [StaticNativePathStep]],
        maximum_occurrences: usize,
    ) -> Result<Self, PreparedNativeRefusal> {
        if N > 4096
            || scopes.len() > 4096
            || maximum_occurrences == 0
            || maximum_occurrences > MAXIMUM_PREPARED_NATIVE_OCCURRENCES
        {
            return Err(PreparedNativeRefusal::Bounds);
        }
        let schema = BorrowedStructuredValueValidator::new(schema, maximum_bytes)
            .map_err(|_| PreparedNativeRefusal::Shape)?;
        let mut predicates = [None; N];
        for path in contracts
            .iter()
            .map(|entry| entry.path)
            .chain(scopes.iter().copied())
        {
            if path.len() > 64
                || path.iter().any(|step| {
                    matches!(
                        step,
                        StaticNativePathStep::Field("") | StaticNativePathStep::Variant("")
                    )
                })
            {
                return Err(PreparedNativeRefusal::InvalidProgram);
            }
        }
        for (index, entry) in contracts.iter().enumerate() {
            predicates[index] = Some(
                BorrowedFixedIntegerContract::new(entry.predicate)
                    .map_err(|_| PreparedNativeRefusal::InvalidProgram)?,
            );
        }
        Ok(Self {
            schema,
            contracts,
            scopes,
            predicates,
            maximum_occurrences,
            intrinsic_empty: false,
        })
    }
    pub fn validate(&self, input: &[u8]) -> Result<(), PreparedNativeRefusal> {
        self.validate_borrowed(input).map(|_| ())
    }
    /// Borrow the representation after all retained checks. This is not an
    /// AdmittedNativeCanonical witness and grants no execution authority.
    pub fn validate_borrowed<'a>(
        &self,
        input: &'a [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, PreparedNativeRefusal> {
        if self.intrinsic_empty && !input.is_empty() {
            return Err(PreparedNativeRefusal::Shape);
        }
        let input = if self.intrinsic_empty {
            INTRINSIC_EMPTY_CANONICAL
        } else {
            input
        };
        let value = self
            .schema
            .validate(input)
            .map_err(|_| PreparedNativeRefusal::Shape)?;
        self.check_contracts(value)
    }
    /// Validate a separately retained exact schema and value node through the
    /// same whole-input Native checks. This creates no admission witness.
    pub fn validate_parts_borrowed<'a>(
        &self,
        schema: &[u8],
        node: &'a [u8],
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, PreparedNativeRefusal> {
        if self.intrinsic_empty || schema != self.schema.type_bytes() {
            return Err(PreparedNativeRefusal::Shape);
        }
        let value = self
            .schema
            .validate_node(node)
            .map_err(|_| PreparedNativeRefusal::Shape)?;
        self.check_contracts(value)
    }
    fn check_contracts<'a>(
        &self,
        value: ValidatedCanonicalStructuredValue<'a>,
    ) -> Result<ValidatedCanonicalStructuredValue<'a>, PreparedNativeRefusal> {
        let mut checked = 0;
        for (index, entry) in self.contracts.iter().enumerate() {
            let predicate = self.predicates[index]
                .as_ref()
                .ok_or(PreparedNativeRefusal::InvalidProgram)?;
            visit_path(
                entry.path,
                value,
                &mut checked,
                self.maximum_occurrences,
                &mut |value| {
                    let bytes = value
                        .primitive_representation_bytes(entry.predicate.value_kind)
                        .map_err(|_| PreparedNativeRefusal::Shape)?;
                    predicate
                        .validate(bytes)
                        .map_err(|refusal| PreparedNativeRefusal::Contract {
                            scope: usize::from(entry.scope),
                            contract: usize::from(entry.contract),
                            refusal,
                        })
                },
            )?;
        }
        // Preserve the same combined contract/native occurrence ceiling, even
        // when this profile's Native scopes contain no Boolean invariants.
        for path in self.scopes {
            visit_path(
                path,
                value,
                &mut checked,
                self.maximum_occurrences,
                &mut |_| Ok(()),
            )?;
        }
        Ok(value)
    }
    /// Cooperative units count bounded canonical/selection/predicate calls,
    /// not CPU instructions or a hard time guarantee. Canonical byte/depth
    /// ceilings and complete stack/workspace require independent Root admission.
    pub fn validation_work(&self, maximum_bytes: usize) -> u32 {
        let nodes = maximum_bytes.min(self.schema.maximum_bytes()) / 5 + 1;
        let visits = |path: &[StaticNativePathStep]| -> u32 {
            if path.iter().any(|s| matches!(s, StaticNativePathStep::Each)) {
                ((path.len() + 1) as u32)
                    .saturating_mul(nodes as u32)
                    .saturating_mul(2)
            } else {
                (path.len() + 1) as u32
            }
        };
        let mut work = 1u32; // one canonical validation with its admitted envelope
        for (i, entry) in self.contracts.iter().enumerate() {
            let leaves = if entry
                .path
                .iter()
                .any(|s| matches!(s, StaticNativePathStep::Each))
            {
                nodes as u32
            } else {
                1
            };
            let checks = self.predicates[i].map_or(u16::MAX, |p| p.validation_work()) as u32;
            work = work
                .saturating_add(1)
                .saturating_add(visits(entry.path))
                .saturating_add(leaves.saturating_mul(checks));
        }
        for path in self.scopes {
            work = work.saturating_add(1).saturating_add(visits(path));
        }
        work
    }
    pub(crate) fn same_definition(&self, other: &Self) -> bool {
        self.intrinsic_empty == other.intrinsic_empty
            && self.schema.type_bytes() == other.schema.type_bytes()
            && self.schema.maximum_bytes() == other.schema.maximum_bytes()
            && core::ptr::eq(self.contracts, other.contracts)
            && core::ptr::eq(self.scopes, other.scopes)
            && self.maximum_occurrences == other.maximum_occurrences
    }
    pub const fn schema_bytes(&self) -> &'static [u8] {
        self.schema.type_bytes()
    }
    pub const fn contract_count(&self) -> usize {
        N
    }
}
// The intrinsic Empty ABI is a zero-byte payload. Its canonical typed view is
// fixed and used only internally for the existing validated path machinery.
const INTRINSIC_EMPTY_TYPE: &[u8] = &[
    0, 11, 0, 0, 0, b'v', b'a', b'l', b'u', b'e', b'/', b'e', b'm', b'p', b't', b'y',
];
const INTRINSIC_EMPTY_CANONICAL: &[u8] = &[
    0, 11, 0, 0, 0, b'v', b'a', b'l', b'u', b'e', b'/', b'e', b'm', b'p', b't', b'y', 0, 0, 0, 0, 0,
];
impl ResidentNativeValidator<0> {
    /// Validate only the standard zero-byte Empty ABI; this is not an authored
    /// Native owner, contract witness, arbitrary schema or admission bypass.
    pub fn intrinsic_empty(maximum_occurrences: usize) -> Result<Self, PreparedNativeRefusal> {
        let mut validator = Self::new(
            INTRINSIC_EMPTY_TYPE,
            INTRINSIC_EMPTY_CANONICAL.len(),
            &[],
            &[],
            maximum_occurrences,
        )?;
        validator.intrinsic_empty = true;
        Ok(validator)
    }
}
impl<const N: usize> ResidentNativeValidator<N> {
    pub(crate) fn is_intrinsic_empty(&self) -> bool {
        self.intrinsic_empty
    }
}
fn visit_path<'a>(
    path: &[StaticNativePathStep],
    value: ValidatedCanonicalStructuredValue<'a>,
    checked: &mut usize,
    maximum: usize,
    check: &mut impl FnMut(ValidatedCanonicalStructuredValue<'a>) -> Result<(), PreparedNativeRefusal>,
) -> Result<(), PreparedNativeRefusal> {
    let shape = |_| PreparedNativeRefusal::Shape;
    let Some((step, remaining)) = path.split_first() else {
        if *checked == maximum {
            return Err(PreparedNativeRefusal::Bounds);
        }
        *checked += 1;
        return check(value);
    };
    match step {
        StaticNativePathStep::Representation => visit_path(
            remaining,
            value.nominal_representation().map_err(shape)?,
            checked,
            maximum,
            check,
        ),
        StaticNativePathStep::Field(name) => visit_path(
            remaining,
            value
                .record_field(name)
                .map_err(shape)?
                .ok_or(PreparedNativeRefusal::Shape)?,
            checked,
            maximum,
            check,
        ),
        StaticNativePathStep::Variant(name) => {
            if let Some(payload) = value.variant_payload(name).map_err(shape)? {
                visit_path(remaining, payload, checked, maximum, check)?;
            }
            Ok(())
        }
        StaticNativePathStep::Each => {
            let count = value.collection_length().map_err(shape)?;
            for index in 0..count {
                let index = u16::try_from(index).map_err(|_| PreparedNativeRefusal::Bounds)?;
                let item = value
                    .collection_index(index)
                    .map_err(shape)?
                    .ok_or(PreparedNativeRefusal::Shape)?;
                visit_path(remaining, item, checked, maximum, check)?;
            }
            Ok(())
        }
    }
}
