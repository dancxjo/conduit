//! Exact selected Millionth Quantity leaf to normalized Scalar conversion.

use alloc::vec::Vec;
use conduit_core::{Quantity, QuantityUnit, Scalar, QUANTITY_ENCODED_LEN};

pub const NORMALIZED_QUANTITY_KIND: &str = "math/normalized-quantity-scalar";
pub const NORMALIZED_QUANTITY_REVISION: &str = "conduit.std/normalized-quantity-scalar@1";

pub fn normalized_quantity_contract() -> crate::StandardKindContract {
    let mut contract = crate::quantity_info_wrap_contract();
    contract.kind_id = conduit_core::kind_id(NORMALIZED_QUANTITY_KIND);
    contract.plain_name = "Normalized Quantity to Scalar".into();
    contract.summary = "Convert an exact Millionth Quantity leaf in [0, 1] to Scalar.".into();
    contract.inputs[0].value_kind = crate::wrapped_quantity_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    contract.outputs[0].value_kind = conduit_core::kind_id(conduit_core::SCALAR_INFO_ID);
    contract.example = "normalize: math/normalized-quantity-scalar".into();
    contract
}

#[cfg(feature = "form-catalog")]
pub fn install_normalized_quantity_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    let contract = normalized_quantity_contract();
    startup.insert(conduit_form::KindSignature {
        kind: NORMALIZED_QUANTITY_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert(conduit_form::KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: NORMALIZED_QUANTITY_REVISION.into(),
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| alloc::format!("{error}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizedQuantityRefusal {
    MalformedOrWrongType,
    IncompatibleUnit,
    OutOfDomain,
}

/// The canonical leaf envelope is admitted once, before execution.
pub struct PreparedNormalizedQuantity {
    prefix: Vec<u8>,
}

impl Default for PreparedNormalizedQuantity {
    fn default() -> Self {
        Self::new()
    }
}

impl PreparedNormalizedQuantity {
    pub fn new() -> Self {
        Self {
            prefix: crate::quantity_info_prefix(),
        }
    }

    /// Prefix equality checks the exact canonical type, shape and leaf length.
    /// Millionth and Scalar microunits are identical integer scales: valid
    /// inputs cannot incur rounding, inexact conversion, or arithmetic overflow.
    pub fn convert(&self, input: &[u8]) -> Result<Scalar, NormalizedQuantityRefusal> {
        if input.len() != self.prefix.len() + QUANTITY_ENCODED_LEN
            || !input.starts_with(&self.prefix)
        {
            return Err(NormalizedQuantityRefusal::MalformedOrWrongType);
        }
        let quantity = Quantity::decode(&input[self.prefix.len()..])
            .map_err(|_| NormalizedQuantityRefusal::MalformedOrWrongType)?;
        if quantity.unit() != QuantityUnit::Millionth {
            return Err(NormalizedQuantityRefusal::IncompatibleUnit);
        }
        if !(0..=1_000_000).contains(&quantity.value()) {
            return Err(NormalizedQuantityRefusal::OutOfDomain);
        }
        Ok(Scalar::from_raw_microunits(quantity.value()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::StructuredInfoValue;

    fn leaf(value: i64, unit: QuantityUnit) -> Vec<u8> {
        StructuredInfoValue::leaf(
            crate::wrapped_quantity_type(),
            Quantity::new(value, unit).encode().to_vec(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    }

    #[test]
    fn normalized_quantity_preserves_exact_microunits() {
        let converter = PreparedNormalizedQuantity::new();
        for value in [0, 1, 250_000, 500_000, 999_999, 1_000_000] {
            assert_eq!(
                converter.convert(&leaf(value, QuantityUnit::Millionth)),
                Ok(Scalar::from_raw_microunits(value))
            );
        }
    }

    #[test]
    fn normalized_quantity_refuses_without_clamping_or_unit_coercion() {
        let converter = PreparedNormalizedQuantity::new();
        for value in [i64::MIN, -1, 1_000_001, i64::MAX] {
            assert_eq!(
                converter.convert(&leaf(value, QuantityUnit::Millionth)),
                Err(NormalizedQuantityRefusal::OutOfDomain)
            );
        }
        for unit in [
            QuantityUnit::One,
            QuantityUnit::Percent,
            QuantityUnit::Hertz,
        ] {
            assert_eq!(
                converter.convert(&leaf(0, unit)),
                Err(NormalizedQuantityRefusal::IncompatibleUnit)
            );
        }
        let valid = leaf(500_000, QuantityUnit::Millionth);
        for length in 0..valid.len() {
            assert_eq!(
                converter.convert(&valid[..length]),
                Err(NormalizedQuantityRefusal::MalformedOrWrongType)
            );
        }
        let mut extra = valid.clone();
        extra.push(0);
        assert!(converter.convert(&extra).is_err());
        // Raw Quantity bytes are not the selector's structured leaf output.
        assert!(converter
            .convert(&Quantity::new(0, QuantityUnit::Millionth).encode())
            .is_err());
        for index in 0..converter.prefix.len() {
            let mut altered = valid.clone();
            altered[index] ^= 1;
            assert_eq!(
                converter.convert(&altered),
                Err(NormalizedQuantityRefusal::MalformedOrWrongType)
            );
        }
    }
}
