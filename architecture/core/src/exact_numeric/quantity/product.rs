//! Finite operation-derived physical families. Binary provenance is intentional:
//! these values do not implicitly acquire a named authored quantity family.
use super::*;
use crate::{PrefixPolicy, QuantityRoles, UnitDefinition};

impl RationalQuantity {
    /// Multiply linear quantities in exact reference coordinates. Product identity
    /// is commutative, but does not assert associative nominal-family rewriting.
    pub fn checked_product(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        self.derived(other, false, work)
    }
    /// Divide linear quantities, preserving both nominal families and origins.
    /// This returns a Quantity even when dimensional exponents cancel.
    pub fn checked_quotient(
        self,
        other: Self,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        self.derived(other, true, work)
    }
    fn derived(
        self,
        other: Self,
        quotient: bool,
        work: &mut ExactNumericWork,
    ) -> Result<Self, RationalQuantityRefusal> {
        if self.role != QuantityRole::Linear || other.role != QuantityRole::Linear {
            return Err(RationalQuantityRefusal::InvalidRole);
        }
        let definition_error =
            |error| RationalQuantityRefusal::Unit(UnitRefusal::Definition(error));
        let dimension = if quotient {
            self.dimension().quotient(other.dimension())
        } else {
            self.dimension().product(other.dimension())
        }
        .map_err(definition_error)?;
        let left = self.reference_coordinate(work)?;
        let right = other.reference_coordinate(work)?;
        let coordinate = if quotient {
            left.checked_divide(right, work)
        } else {
            left.checked_multiply(right, work)
        }?;
        let mut operands = [
            (self.family().identity(), self.unit.reference_anchor()),
            (other.family().identity(), other.unit.reference_anchor()),
        ];
        if !quotient {
            operands.sort_unstable();
        }
        let mut family_facts = [0u8; 65];
        family_facts[0] = u8::from(quotient);
        family_facts[1..33].copy_from_slice(&operands[0].0);
        family_facts[33..].copy_from_slice(&operands[1].0);
        let family_name = digest_name(crate::semantic_digest(
            "physical/quantity-product-family@1",
            &family_facts,
        ));
        let family = QuantityFamilyDefinition::new(
            core::str::from_utf8(&family_name).expect("hex digest"),
            dimension,
            QuantityRoles::Linear,
        )
        .map_err(definition_error)?;
        let mut unit_facts = [0u8; 129];
        unit_facts[..65].copy_from_slice(&family_facts);
        unit_facts[65..97].copy_from_slice(&operands[0].1);
        unit_facts[97..].copy_from_slice(&operands[1].1);
        let symbol = digest_name(crate::semantic_digest(
            "physical/quantity-product-origin@1",
            &unit_facts,
        ));
        let definition = UnitDefinition::new(
            family,
            core::str::from_utf8(&symbol).expect("hex digest"),
            1,
            0,
            1,
            0,
            PrefixPolicy::NONE,
        )
        .map_err(definition_error)?;
        Self::new(
            coordinate,
            Unit::from_definition(definition),
            QuantityRole::Linear,
        )
    }
}
fn digest_name(digest: [u8; 32]) -> [u8; 64] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut name = [0; 64];
    for (index, byte) in digest.into_iter().enumerate() {
        name[index * 2] = HEX[usize::from(byte >> 4)];
        name[index * 2 + 1] = HEX[usize::from(byte & 15)];
    }
    name
}
