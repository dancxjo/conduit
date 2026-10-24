//! Distinct Kind/Fore identity for the complete checked-law capsule.
use super::*;
use crate::{KindConfigurationField, KindConfigurationRule, KindProjection};
use alloc::{format, string::String, vec};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
};
use sha2::{Digest, Sha256};
pub const LAW_EXPRESSION_REVISION: &str = "conduitese/law-expression-operation@1";
pub const LAW_FILTER_REVISION: &str = "conduitese/law-filter-operation@1";
pub fn law_expression_definition(
    material: &LawExpressionMaterial,
    temporal: PortTemporal,
) -> Result<KindProjection, LawExpressionRefusal> {
    definition(material, temporal, false)
}
pub fn law_filter_definition(
    material: &LawExpressionMaterial,
    temporal: PortTemporal,
) -> Result<KindProjection, LawExpressionRefusal> {
    definition(material, temporal, true)
}
fn definition(
    material: &LawExpressionMaterial,
    temporal: PortTemporal,
    filter: bool,
) -> Result<KindProjection, LawExpressionRefusal> {
    if temporal == PortTemporal::Current {
        return Err(LawExpressionRefusal::Profile);
    }
    let input = crate::expression_definition::expression_port_kind(&material.program.input_type)
        .map_err(|_| LawExpressionRefusal::Encoding)?;
    let output = if filter {
        if material.program.output_type
            != conduit_core::StructuredInfoType::leaf(kind_id(conduit_core::BOOL_INFO_ID))
                .map_err(|_| LawExpressionRefusal::Encoding)?
        {
            return Err(LawExpressionRefusal::Encoding);
        }
        if temporal == PortTemporal::Value {
            conduit_core::optional_info_type(material.program.input_type.clone())
                .map_err(|_| LawExpressionRefusal::Encoding)?
                .profile()
                .map_err(|_| LawExpressionRefusal::Encoding)?
                .value_kind()
                .clone()
        } else {
            input.clone()
        }
    } else {
        crate::expression_definition::expression_port_kind(&material.program.output_type)
            .map_err(|_| LawExpressionRefusal::Encoding)?
    };
    let encoded = hex(&material.canonical_bytes()?);
    let revision = if filter {
        LAW_FILTER_REVISION
    } else {
        LAW_EXPRESSION_REVISION
    };
    let name = if filter {
        "law-filter"
    } else {
        "law-expression"
    };
    let mut digest = Sha256::new();
    for bytes in [
        name,
        revision,
        LAW_EXPRESSION_PROFILE,
        input.as_str(),
        output.as_str(),
        temporal.as_str(),
        &encoded,
    ] {
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes.as_bytes());
    }
    Ok(KindProjection {
        kind_id: kind_id(&format!("conduitese/{name}/{:x}", digest.finalize())),
        kind_contract_revision: KindIdentity::from(revision),
        inputs: vec![PortDescriptor {
            port_id: port_id("input"),
            value_kind: input,
            direction: PortDirection::Input,
            temporal,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("output"),
            value_kind: output,
            direction: PortDirection::Output,
            temporal,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "law-program".into(),
            default_value: ConfigurationValue::Text(encoded),
            rule: KindConfigurationRule::TextBytes {
                maximum: (MAXIMUM_LAW_EXPRESSION_BYTES * 2) as u32,
            },
        }],
    })
}
fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(HEX[(byte >> 4) as usize] as char);
        value.push(HEX[(byte & 15) as usize] as char);
    }
    value
}
