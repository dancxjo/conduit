//! Exact selected Millionth Quantity leaf to normalized Scalar conversion.

use alloc::vec::Vec;
use conduit_core::{Kind, Quantity, Scalar, Unit};
pub use conduit_data::NormalizedQuantityRefusal;

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
    contract.outputs[0].value_kind = conduit_core::kind_id(&conduit_core::PortableNumericProfile { maximum_encoded_bytes: crate::QUANTITY_INFO_MAXIMUM_BYTES as u32, ..conduit_core::PortableNumericProfile::MAXIMAL }.info_id().expect("bounded Scalar"));
    contract.example = "normalize: math/normalized-quantity-scalar".into();
    contract
}

pub fn normalized_quantity_semantic_contract() -> Kind {
    let contract = normalized_quantity_contract();
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: NORMALIZED_QUANTITY_REVISION.into(),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![conduit_core::KindSemanticLaw::Terminal(
            contract.terminal_behavior
        )],
        limits: contract.limits,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_normalized_quantity_catalog(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: NORMALIZED_QUANTITY_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    startup.insert(conduit_plot::KindSignature {
        kind: NORMALIZED_RATIO_KIND.into(),
        startup_parameters: Vec::new(),
    })?;
    profile
        .insert_kind(normalized_ratio_semantic_contract())
        .map_err(|error| alloc::format!("{error}"))?;
    profile
        .insert_kind(normalized_quantity_semantic_contract())
        .map_err(|error| alloc::format!("{error}"))
}

/// Convert the exact Ratio leaf emitted by normalized input surfaces.
pub const NORMALIZED_RATIO_KIND: &str = "math/normalized-ratio-scalar";
pub const NORMALIZED_RATIO_REVISION: &str = "conduit.std/normalized-ratio-scalar@1";

pub fn normalized_ratio_semantic_contract() -> Kind {
    let mut contract = normalized_quantity_semantic_contract();
    contract.kind_id = conduit_core::kind_id(NORMALIZED_RATIO_KIND);
    contract.kind_contract_revision = NORMALIZED_RATIO_REVISION.into();
    contract.inputs[0].value_kind =
        conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::RATIO_INFO_ID))
            .expect("canonical Ratio leaf")
            .profile()
            .unwrap()
            .value_kind()
            .clone();
    contract
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

    pub fn ratio() -> Self {
        let mut prefix = conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
            conduit_core::RATIO_INFO_ID,
        ))
        .expect("Ratio type")
        .canonical_bytes()
        .expect("checked type");
        prefix.push(0);
        Self { prefix }
    }

    /// Prefix equality checks the exact canonical type, shape and leaf length.
    /// Millionth and Scalar microunits are identical integer scales: valid
    /// inputs cannot incur rounding, inexact conversion, or arithmetic overflow.
    pub fn convert(&self, input: &[u8]) -> Result<Scalar, NormalizedQuantityRefusal> {
        let node = input
            .strip_prefix(self.prefix.as_slice())
            .ok_or(NormalizedQuantityRefusal::MalformedOrWrongType)?;
        let length_bytes: [u8; 4] = node
            .get(..4)
            .and_then(|x| x.try_into().ok())
            .ok_or(NormalizedQuantityRefusal::MalformedOrWrongType)?;
        let length = u32::from_le_bytes(length_bytes) as usize;
        let payload = node
            .get(4..)
            .ok_or(NormalizedQuantityRefusal::MalformedOrWrongType)?;
        if payload.len() != length || input.len() > crate::QUANTITY_INFO_MAXIMUM_BYTES {
            return Err(NormalizedQuantityRefusal::MalformedOrWrongType);
        }
        let quantity = Quantity::decode(payload)
            .map_err(|_| NormalizedQuantityRefusal::MalformedOrWrongType)?;
        if !quantity
            .unit()
            .same_physical_definition(Unit::Millionth.as_ref())
        {
            return Err(NormalizedQuantityRefusal::IncompatibleUnit);
        }
        let coordinate = quantity
            .convert_to_unit(&Unit::One)
            .map_err(|_| NormalizedQuantityRefusal::OutOfDomain)?;
        if coordinate
            .compare(&Scalar::ZERO)
            .map_err(|_| NormalizedQuantityRefusal::OutOfDomain)?
            == Some(core::cmp::Ordering::Less)
            || coordinate
                .compare(&Scalar::ONE)
                .map_err(|_| NormalizedQuantityRefusal::OutOfDomain)?
                != Some(core::cmp::Ordering::Less)
                && coordinate != Scalar::ONE
        {
            return Err(NormalizedQuantityRefusal::OutOfDomain);
        }
        if coordinate.canonical_bytes().map_err(|_| NormalizedQuantityRefusal::OutOfDomain)?.len() > crate::QUANTITY_INFO_MAXIMUM_BYTES { return Err(NormalizedQuantityRefusal::OutOfDomain); }
        Ok(coordinate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::StructuredInfoValue;

    fn leaf(value: i64, unit: Unit) -> Vec<u8> {
        StructuredInfoValue::leaf(
            crate::wrapped_quantity_type(),
            Quantity::new(value, unit.clone()).encode().to_vec(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap()
    }

    #[test]
    fn normalized_ratio_preserves_bounds_and_refuses_quantity_identity() {
        let converter = PreparedNormalizedQuantity::ratio();
        let ratio = |value: i64, unit: Unit| {
            StructuredInfoValue::leaf(
                conduit_core::StructuredInfoType::leaf(conduit_core::kind_id(
                    conduit_core::RATIO_INFO_ID,
                ))
                .unwrap(),
                Quantity::new(value, unit.clone()).encode().to_vec(),
            )
            .unwrap()
            .canonical_bytes()
            .unwrap()
        };
        for value in [0, 1, 250_000, 999_999, 1_000_000] {
            assert_eq!(
                converter.convert(&ratio(value, Unit::Millionth)),
                Ok(Scalar::from_decimal_parts(i128::from(value), -6).unwrap())
            );
        }
        for value in [-1, 1_000_001] {
            assert_eq!(
                converter.convert(&ratio(value, Unit::Millionth)),
                Err(NormalizedQuantityRefusal::OutOfDomain)
            );
        }
        assert_eq!(
            converter.convert(&ratio(1, Unit::Percent)),
            Err(NormalizedQuantityRefusal::IncompatibleUnit)
        );
        assert_eq!(
            converter.convert(&leaf(250_000, Unit::Millionth)),
            Err(NormalizedQuantityRefusal::MalformedOrWrongType)
        );
        assert_eq!(
            PreparedNormalizedQuantity::new().convert(&ratio(250_000, Unit::Millionth)),
            Err(NormalizedQuantityRefusal::MalformedOrWrongType)
        );
        let bytes = ratio(250_000, Unit::Millionth);
        for length in 0..bytes.len() {
            assert_eq!(
                converter.convert(&bytes[..length]),
                Err(NormalizedQuantityRefusal::MalformedOrWrongType)
            );
        }
    }

    #[test]
    fn normalized_quantity_preserves_exact_microunits() {
        let converter = PreparedNormalizedQuantity::new();
        for value in [0, 1, 250_000, 500_000, 999_999, 1_000_000] {
            assert_eq!(
                converter.convert(&leaf(value, Unit::Millionth)),
                Ok(Scalar::from_decimal_parts(i128::from(value), -6).unwrap())
            );
        }
    }

    #[test]
    fn normalized_quantity_refuses_without_clamping_or_unit_coercion() {
        let converter = PreparedNormalizedQuantity::new();
        for value in [i64::MIN, -1, 1_000_001, i64::MAX] {
            assert_eq!(
                converter.convert(&leaf(value, Unit::Millionth)),
                Err(NormalizedQuantityRefusal::OutOfDomain)
            );
        }
        for unit in [Unit::One, Unit::Percent, Unit::Hertz] {
            assert_eq!(
                converter.convert(&leaf(0, unit)),
                Err(NormalizedQuantityRefusal::IncompatibleUnit)
            );
        }
        let valid = leaf(500_000, Unit::Millionth);
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
            .convert(&Quantity::new(0, Unit::Millionth).encode())
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

    #[test]
    fn semantic_contract_owns_normalized_quantity_identity_and_capacity() {
        let contract = normalized_quantity_semantic_contract();
        assert_eq!(
            contract.kind_contract_revision.as_str(),
            NORMALIZED_QUANTITY_REVISION
        );
        assert!(contract.startup_parameters.is_empty());
        assert_eq!(contract.limits, normalized_quantity_contract().limits);
    }
}
