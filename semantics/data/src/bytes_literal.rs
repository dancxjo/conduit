//! Immutable authored octets. Hex is preparation-only configuration, not I/O.
use alloc::{string::String, vec, vec::Vec};
use conduit_core::*;
pub const BYTES_LITERAL_KIND: &str = "bytes/literal";
pub const BYTES_LITERAL_REVISION: &str = "conduit.std/bytes-literal@1";
pub const MAXIMUM_LITERAL_BYTES: u32 = 2048;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BytesLiteralRefusal {
    OddLength,
    InvalidHex,
    Oversized,
}
pub fn validate_hex(hex: &str) -> Result<usize, BytesLiteralRefusal> {
    if hex.len() > MAXIMUM_LITERAL_BYTES as usize * 2 {
        return Err(BytesLiteralRefusal::Oversized);
    }
    if !hex.len().is_multiple_of(2) {
        return Err(BytesLiteralRefusal::OddLength);
    }
    if !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(BytesLiteralRefusal::InvalidHex);
    }
    Ok(hex.len() / 2)
}
pub fn decode_hex(hex: &str) -> Result<Vec<u8>, BytesLiteralRefusal> {
    let length = validate_hex(hex)?;
    let nibble = |byte: u8| {
        if byte.is_ascii_digit() {
            byte - b'0'
        } else {
            byte.to_ascii_lowercase() - b'a' + 10
        }
    };
    let mut bytes = Vec::with_capacity(length);
    for pair in hex.as_bytes().as_chunks::<2>().0 {
        bytes.push((nibble(pair[0]) << 4) | nibble(pair[1]));
    }
    Ok(bytes)
}
pub fn definition() -> Kind {
    let output = PortDescriptor {
        port_id: port_id("value"),
        value_kind: kind_id("value/bytes"),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(BYTES_LITERAL_KIND),
        kind_contract_revision: KindIdentity::from(BYTES_LITERAL_REVISION),
        startup_parameters: vec![FrontStartupParameter {
            name: "hex".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![],
        outputs: vec![output.clone()],
        configuration: vec![KindConfigurationField {
            key: "hex".into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes {
                maximum: MAXIMUM_LITERAL_BYTES * 2,
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce),
            KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: FrontValueLocation::Output(output.port_id),
                contract: CheckedValueContract::new(
                    output.value_kind,
                    MAXIMUM_LITERAL_BYTES,
                    vec![],
                )
                .expect("bounded byte literal"),
            }]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_LITERAL_BYTES,
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_octets_and_finite_bounds_are_preparation_only() {
        assert_eq!(decode_hex("00ff80aA").unwrap(), [0, 255, 128, 170]);
        assert_eq!(decode_hex("").unwrap(), []);
        assert_eq!(validate_hex("0"), Err(BytesLiteralRefusal::OddLength));
        assert_eq!(validate_hex("00 1"), Err(BytesLiteralRefusal::InvalidHex));
        assert_eq!(validate_hex("zz"), Err(BytesLiteralRefusal::InvalidHex));
        assert_eq!(
            validate_hex(&"00".repeat(2049)),
            Err(BytesLiteralRefusal::Oversized)
        );
        assert_eq!(decode_hex(&"80".repeat(2048)).unwrap().len(), 2048);
    }
    #[test]
    fn literal_has_no_input_or_inferred_effect_authority() {
        let kind = definition();
        assert!(kind.inputs.is_empty());
        assert_eq!(kind.outputs[0].value_kind.as_str(), "value/bytes");
        assert_eq!(kind.outputs[0].temporal, PortTemporal::Value);
        let fore = kind.checked_front();
        let output = fore
            .value_contracts()
            .iter()
            .find(|value| {
                value.location == FrontValueLocation::Output(kind.outputs[0].port_id.clone())
            })
            .expect("exact bounded byte output");
        assert_eq!(output.contract.maximum_bytes, MAXIMUM_LITERAL_BYTES);
        assert_eq!(output.contract.value_kind, kind.outputs[0].value_kind);
    }
}
