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
pub struct ResidentNativeValidator<const CONTRACTS: usize> {
    schema: BorrowedStructuredValueValidator<'static>,
    contracts: &'static [StaticNativeContract; CONTRACTS],
    scopes: &'static [&'static [StaticNativePathStep]],
    predicates: [Option<BorrowedFixedIntegerContract<'static>>; CONTRACTS],
    maximum_occurrences: usize,
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
        })
    }
    pub fn validate(&self, input: &[u8]) -> Result<(), PreparedNativeRefusal> {
        let value = self
            .schema
            .validate(input)
            .map_err(|_| PreparedNativeRefusal::Shape)?;
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
        Ok(())
    }
    pub const fn schema_bytes(&self) -> &'static [u8] {
        self.schema.type_bytes()
    }
    pub const fn contract_count(&self) -> usize {
        N
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
