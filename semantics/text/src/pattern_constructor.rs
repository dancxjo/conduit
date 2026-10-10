//! One ordinary checked pattern-specification constructor and its glyph adapter.
use crate::{PortablePatternSpecification, PortablePatternSpecificationRefusal};
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{
    rust_binding::NativeRustBinding, KindSignature, LiteralValueConstructor, ProfileCatalog,
    StartupCatalog, StartupParameterSignature, StaticValueConstructor, TypedGlyphLiteralSyntax,
    TypedLiteralLexicalPolicy,
};

pub const PATTERN_CONSTRUCTOR_KIND: &str = "text/pattern-from-source";
pub const PATTERN_CONSTRUCTOR_REVISION: &str = "conduit.text/portable-pattern@1";
const FIELDS: [&str; 4] = [
    "source",
    "case-insensitive",
    "anchored-start",
    "anchored-end",
];

#[derive(Debug, Clone, Copy)]
pub struct PortablePatternConstructor;
#[derive(Debug)]
pub enum PatternConstructorRefusal {
    Configuration,
    Specification(PortablePatternSpecificationRefusal),
    Codec(conduit_plot::rust_binding::NativeBindingRefusal),
    Core(StructuredInfoRefusal),
}
impl StaticValueConstructor for PortablePatternConstructor {
    type Refusal = PatternConstructorRefusal;
    fn contract(&self) -> Kind {
        Kind {
            kind_id: kind_id(PATTERN_CONSTRUCTOR_KIND),
            kind_contract_revision: KindIdentity::from(PATTERN_CONSTRUCTOR_REVISION),
            startup_parameters: FIELDS
                .iter()
                .enumerate()
                .map(|(i, name)| FrontStartupParameter {
                    name: (*name).into(),
                    value_type: kind_id(if i == 0 { TEXT_INFO_ID } else { BOOL_INFO_ID }),
                    has_default: false,
                })
                .collect(),
            shorthand: None,
            inputs: vec![],
            outputs: vec![PortDescriptor {
                port_id: port_id("value"),
                value_kind: self
                    .result_type()
                    .profile()
                    .expect("finite specification")
                    .value_kind()
                    .clone(),
                direction: PortDirection::Output,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            configuration: FIELDS
                .iter()
                .enumerate()
                .map(|(i, name)| KindConfigurationField {
                    key: (*name).into(),
                    default_value: if i == 0 {
                        ConfigurationValue::Text("a".into())
                    } else {
                        ConfigurationValue::Bool(false)
                    },
                    rule: if i == 0 {
                        KindConfigurationRule::TextBytes { maximum: 4096 }
                    } else {
                        KindConfigurationRule::Any
                    },
                })
                .collect(),
            semantic_laws: vec![KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce)],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            },
        }
    }
    fn result_type(&self) -> StructuredInfoType {
        PortablePatternSpecification::semantic_type().expect("checked Source specification Type")
    }
    fn prepare_configuration(
        &self,
        configuration: &[ConfigurationEntry],
    ) -> Result<StructuredInfoValue, Self::Refusal> {
        if configuration.len() != FIELDS.len() {
            return Err(PatternConstructorRefusal::Configuration);
        }
        let mut values = Vec::new();
        for name in FIELDS {
            let mut matches = configuration.iter().filter(|entry| entry.key == name);
            let entry = matches
                .next()
                .ok_or(PatternConstructorRefusal::Configuration)?;
            if matches.next().is_some() {
                return Err(PatternConstructorRefusal::Configuration);
            }
            values.push(&entry.value);
        }
        let [ConfigurationValue::Text(source), ConfigurationValue::Bool(insensitive), ConfigurationValue::Bool(start), ConfigurationValue::Bool(end)] =
            values.as_slice()
        else {
            return Err(PatternConstructorRefusal::Configuration);
        };
        if source.len() > conduit_plot::MAXIMUM_TEXT_PATTERN_SOURCE_BYTES {
            return Err(PatternConstructorRefusal::Configuration);
        }
        let specification =
            PortablePatternSpecification::new(source.clone(), *insensitive, *start, *end)
                .map_err(PatternConstructorRefusal::Specification)?;
        let encoded = specification
            .encode()
            .map_err(PatternConstructorRefusal::Codec)?;
        StructuredInfoValue::from_canonical_bytes(&encoded).map_err(PatternConstructorRefusal::Core)
    }
}
impl LiteralValueConstructor for PortablePatternConstructor {
    fn parser_contract(&self) -> &str {
        PATTERN_CONSTRUCTOR_REVISION
    }
    fn lexical_policy(&self) -> TypedLiteralLexicalPolicy {
        TypedLiteralLexicalPolicy::PortablePattern
    }
    fn literal_configuration(
        &self,
        literal: &TypedGlyphLiteralSyntax,
        context: &[ConfigurationEntry],
    ) -> Result<Vec<ConfigurationEntry>, Self::Refusal> {
        if !context.is_empty() {
            return Err(PatternConstructorRefusal::Configuration);
        }
        Ok(vec![
            ConfigurationEntry {
                key: FIELDS[0].into(),
                value: ConfigurationValue::Text(literal.payload.clone()),
            },
            ConfigurationEntry {
                key: FIELDS[1].into(),
                value: ConfigurationValue::Bool(literal.case_insensitive),
            },
            ConfigurationEntry {
                key: FIELDS[2].into(),
                value: ConfigurationValue::Bool(literal.anchored_start),
            },
            ConfigurationEntry {
                key: FIELDS[3].into(),
                value: ConfigurationValue::Bool(literal.anchored_end),
            },
        ])
    }
}
/// Install the ordinary owner and the actual checked Source shipment atomically.
pub fn install_pattern_notation(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let mut next_startup = startup.clone();
    let mut next_profile = profile.clone();
    if next_startup
        .structured_type_name(
            PortablePatternConstructor
                .result_type()
                .profile()
                .expect("finite specification")
                .value_kind(),
        )
        .is_none()
    {
        crate::install_portable_pattern_type(&mut next_startup)?;
    }
    next_startup.insert(KindSignature {
        kind: PATTERN_CONSTRUCTOR_KIND.into(),
        startup_parameters: FIELDS
            .iter()
            .enumerate()
            .map(|(i, name)| StartupParameterSignature {
                name: (*name).into(),
                value_type: if i == 0 { "Text" } else { "Boolean" }.into(),
                default: None,
            })
            .collect(),
    })?;
    next_profile
        .insert_kind(PortablePatternConstructor.contract())
        .map_err(|e| format!("{e}"))?;
    super::pattern_notation::install(&mut next_startup, &next_profile)?;
    next_startup.install_literal_constructor(&PortablePatternConstructor, &next_profile)?;
    *startup = next_startup;
    *profile = next_profile;
    Ok(())
}
