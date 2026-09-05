//! Configuration and bounded transformation for the installed quantity mapper.

use conduit_core::{ConfigurationValue, PlannedGear, QuantityUnit, Scalar, QUANTITY_ENCODED_LEN};
use conduit_kernel::{Failure, FailureCode};
use conduit_semantic_catalog::{
    QuantityMapping, QuantityMappingRefusal, QuantizationPolicy, RangePolicy,
};

pub(super) fn configuration(placement: &PlannedGear) -> Result<QuantityMapping, String> {
    let number = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|field| {
                if field.key == key {
                    if let ConfigurationValue::I64(value) = field.value {
                        return Some(value);
                    }
                }
                None
            })
            .ok_or_else(|| format!("quantity mapping requires integer '{key}'"))
    };
    let text = |key: &str| {
        placement
            .configuration
            .iter()
            .find_map(|field| {
                if field.key == key {
                    if let ConfigurationValue::Text(value) = &field.value {
                        return Some(value.as_str());
                    }
                }
                None
            })
            .ok_or_else(|| format!("quantity mapping requires text '{key}'"))
    };
    QuantityMapping {
        source_minimum: Scalar::from_raw_microunits(number("source-minimum")?),
        source_maximum: Scalar::from_raw_microunits(number("source-maximum")?),
        target_minimum: number("target-minimum")?,
        target_maximum: number("target-maximum")?,
        target_granularity: number("target-granularity")?,
        target_unit: QuantityUnit::from_form_suffix(text("unit")?)
            .map_err(|error| format!("quantity mapping unit: {error:?}"))?,
        range_policy: match text("range-policy")? {
            "refuse" => RangePolicy::Refuse,
            "clamp" => RangePolicy::Clamp,
            _ => return Err("unknown quantity mapping range policy".into()),
        },
        quantization: match text("quantization")? {
            "exact" => QuantizationPolicy::Exact,
            "nearest" => QuantizationPolicy::Nearest,
            _ => return Err("unknown quantity mapping quantization policy".into()),
        },
    }
    .validate()
    .map_err(|error| format!("quantity mapping configuration: {error:?}"))
}

pub(super) fn transform(
    mapping: QuantityMapping,
    input: &[u8],
) -> Result<[u8; QUANTITY_ENCODED_LEN], Failure> {
    let input = Scalar::decode(input).map_err(|_| Failure {
        code: FailureCode::InvalidInput,
        detail: 1,
    })?;
    mapping
        .map(input)
        .map(|quantity| quantity.encode())
        .map_err(|error| Failure {
            code: FailureCode::InvalidInput,
            detail: match error {
                QuantityMappingRefusal::InvalidRange => 2,
                QuantityMappingRefusal::OutOfRange => 3,
                QuantityMappingRefusal::Inexact => 4,
                QuantityMappingRefusal::Overflow => 5,
            },
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::Quantity;

    fn mapping() -> QuantityMapping {
        QuantityMapping {
            source_minimum: Scalar::ZERO,
            source_maximum: Scalar::ONE,
            target_minimum: 0,
            target_maximum: 100,
            target_granularity: 1,
            target_unit: QuantityUnit::Percent,
            range_policy: RangePolicy::Refuse,
            quantization: QuantizationPolicy::Exact,
        }
    }

    #[test]
    fn host_encoding_preserves_quantity_unit_and_distinct_refusals() {
        let value = transform(mapping(), &Scalar::from_raw_microunits(500_000).encode()).unwrap();
        assert_eq!(
            Quantity::decode(&value),
            Ok(Quantity::new(50, QuantityUnit::Percent))
        );
        let malformed = transform(mapping(), &[0]).unwrap_err();
        let outside = transform(mapping(), &Scalar::from_raw_microunits(-1).encode()).unwrap_err();
        let inexact = transform(mapping(), &Scalar::from_raw_microunits(1).encode()).unwrap_err();
        assert_eq!(
            (malformed.detail, outside.detail, inexact.detail),
            (1, 3, 4)
        );
        assert_eq!(outside.code, FailureCode::InvalidInput);
        assert_eq!(inexact.code, FailureCode::InvalidInput);
    }
}
