//! Exact finite schema custody for prepared payload-only structured Flow pairing.
use alloc::{string::String, vec::Vec};
use conduit_core::{
    CheckedValueContract, ConfigurationEntry, ConfigurationValue, Kind, KindConfigurationField,
    KindConfigurationRule, StructuredInfoType,
};
pub const FLOW_ZIP_MAXIMUM_SCHEMA_BYTES: usize = 4096;
fn encode(ty: &StructuredInfoType) -> Result<String, &'static str> {
    let bytes = ty
        .canonical_bytes()
        .map_err(|_| "invalid flow/zip schema")?;
    if bytes.len() > FLOW_ZIP_MAXIMUM_SCHEMA_BYTES {
        return Err("flow/zip schema exceeds its finite bound");
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push(HEX[(byte >> 4) as usize] as char);
        text.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(text)
}
fn decode(value: &ConfigurationValue) -> Result<StructuredInfoType, &'static str> {
    let ConfigurationValue::Text(text) = value else {
        return Err("flow/zip schema must be canonical hexadecimal text");
    };
    if text.len() > FLOW_ZIP_MAXIMUM_SCHEMA_BYTES * 2 || text.len() % 2 != 0 {
        return Err("flow/zip schema exceeds its finite encoding profile");
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("flow/zip schema has noncanonical hexadecimal"),
    };
    let bytes = text
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| Ok((digit(pair[0])? << 4) | digit(pair[1])?))
        .collect::<Result<Vec<_>, &'static str>>()?;
    let ty = StructuredInfoType::from_canonical_bytes(&bytes)
        .map_err(|_| "malformed flow/zip schema")?;
    if encode(&ty)? != *text {
        return Err("noncanonical flow/zip schema");
    }
    Ok(ty)
}
/// Extend exact typed pairing with immutable canonical schemas in the Kind's
/// prepared configuration. This does not select a Host or implementation.
pub fn flow_zip_schema_semantic_contract(
    left: &CheckedValueContract,
    left_type: &StructuredInfoType,
    right: &CheckedValueContract,
    right_type: &StructuredInfoType,
) -> Result<Kind, &'static str> {
    let mut kind = super::flow_zip_finite_semantic_contract(left, left_type, right, right_type)?;
    let paired_type =
        conduit_core::tuple_info_type(alloc::vec![left_type.clone(), right_type.clone()])
            .map_err(|_| "invalid flow/zip tuple type")?;
    let digest = paired_type
        .semantic_digest()
        .map_err(|_| "invalid flow/zip type identity")?;
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut suffix = String::with_capacity(digest.len() * 2);
    for byte in digest {
        suffix.push(HEX[(byte >> 4) as usize] as char);
        suffix.push(HEX[(byte & 15) as usize] as char);
    }
    kind.kind_id = conduit_core::KindId::from(alloc::format!("flow/zip/schema-{suffix}"));
    kind.kind_contract_revision =
        conduit_core::KindIdentity::from(alloc::format!("conduit.flow/zip-schema-{suffix}@1"));

    for (key, ty) in [("left-schema", left_type), ("right-schema", right_type)] {
        kind.startup_parameters
            .push(conduit_core::FrontStartupParameter {
                name: key.into(),
                value_type: conduit_core::kind_id("value/text"),
                has_default: true,
            });
        kind.configuration.push(KindConfigurationField {
            key: key.into(),
            default_value: ConfigurationValue::Text(encode(ty)?),
            rule: KindConfigurationRule::TextBytes {
                maximum: (FLOW_ZIP_MAXIMUM_SCHEMA_BYTES * 2) as u32,
            },
        });
    }
    kind.validate()
        .map_err(|_| "invalid canonical flow/zip schema Kind")?;
    Ok(kind)
}
/// Opaque legacy pairing has no configuration. Typed pairing requires both
/// schemas exactly once; unknown or partial fields never become opaque fallback.
pub fn flow_zip_prepared_schemas(
    configuration: &[ConfigurationEntry],
) -> Result<Option<(StructuredInfoType, StructuredInfoType)>, &'static str> {
    if configuration.is_empty() {
        return Ok(None);
    }
    if configuration.len() != 2 {
        return Err("flow/zip requires two exact schemas");
    }
    let field = |name| {
        let mut matches = configuration.iter().filter(|entry| entry.key == name);
        let entry = matches.next().ok_or("missing flow/zip schema")?;
        if matches.next().is_some() {
            return Err("duplicate flow/zip schema");
        }
        decode(&entry.value)
    };
    Ok(Some((field("left-schema")?, field("right-schema")?)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, StructuredFieldType};
    fn fixture() -> (StructuredInfoType, CheckedValueContract) {
        let ty = StructuredInfoType::record(
            kind_id("type/schema-custody-fixture@1"),
            alloc::vec![StructuredFieldType::new(
                "sample",
                StructuredInfoType::leaf(kind_id("value/i32")).unwrap()
            )
            .unwrap()],
        )
        .unwrap();
        let contract = CheckedValueContract::new(
            ty.profile().unwrap().value_kind().clone(),
            512,
            alloc::vec![],
        )
        .unwrap();
        (ty, contract)
    }
    #[test]
    fn canonical_schemas_survive_preparation_without_replacing_port_truth() {
        let (ty, contract) = fixture();
        let kind = flow_zip_schema_semantic_contract(&contract, &ty, &contract, &ty).unwrap();
        kind.validate().unwrap();
        assert_eq!(kind.startup_parameters.len(), 2);
        assert_eq!(kind.inputs[0].value_kind, contract.value_kind);
        assert_eq!(kind.inputs[1].value_kind, contract.value_kind);
        let configuration: Vec<_> = kind
            .configuration
            .iter()
            .map(|field| ConfigurationEntry {
                key: field.key.clone(),
                value: field.default_value.clone(),
            })
            .collect();
        assert_eq!(
            flow_zip_prepared_schemas(&configuration).unwrap(),
            Some((ty.clone(), ty))
        );
        assert_eq!(flow_zip_prepared_schemas(&[]).unwrap(), None);
    }
    #[test]
    fn nested_pairing_has_distinct_kind_identity_and_retains_exact_ports() {
        let (ty, contract) = fixture();
        let first = flow_zip_schema_semantic_contract(&contract, &ty, &contract, &ty).unwrap();
        let pair = conduit_core::tuple_info_type(alloc::vec![ty.clone(), ty]).unwrap();
        let pair_contract = CheckedValueContract::new(
            pair.profile().unwrap().value_kind().clone(),
            2048,
            alloc::vec![],
        )
        .unwrap();
        let second =
            flow_zip_schema_semantic_contract(&pair_contract, &pair, &pair_contract, &pair)
                .unwrap();
        assert_ne!(first.kind_id, second.kind_id);
        assert_ne!(first.kind_contract_revision, second.kind_contract_revision);
        assert_eq!(second.inputs[0].value_kind, first.outputs[0].value_kind);
        assert!(first
            .inputs
            .iter()
            .chain(&first.outputs)
            .all(|port| port.abnormal_kind.is_none()));
        assert_eq!(first.terminal_transductions().count(), 2);
    }
    #[test]
    fn partial_forged_and_unbounded_schema_configuration_refuses() {
        let (ty, contract) = fixture();
        let kind = flow_zip_schema_semantic_contract(&contract, &ty, &contract, &ty).unwrap();
        let configuration: Vec<_> = kind
            .configuration
            .iter()
            .map(|field| ConfigurationEntry {
                key: field.key.clone(),
                value: field.default_value.clone(),
            })
            .collect();
        assert!(flow_zip_prepared_schemas(&configuration[..1]).is_err());
        let mut duplicate = configuration.clone();
        duplicate[1].key = duplicate[0].key.clone();
        assert!(flow_zip_prepared_schemas(&duplicate).is_err());
        let mut unknown = configuration.clone();
        unknown[1].key = "unexpected".into();
        assert!(flow_zip_prepared_schemas(&unknown).is_err());
        for text in [
            "zz".into(),
            "0".into(),
            "00".into(),
            "a".repeat(FLOW_ZIP_MAXIMUM_SCHEMA_BYTES * 2 + 2),
        ] {
            let mut corrupt = configuration.clone();
            corrupt[0].value = ConfigurationValue::Text(text);
            assert!(flow_zip_prepared_schemas(&corrupt).is_err());
        }
    }
}
