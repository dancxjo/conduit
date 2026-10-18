//! Typed explicit IEEE semantics. Availability belongs to concrete Host offers.
use super::named;
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{KindSignature, ProfileCatalog, StartupCatalog, StartupParameterSignature};

pub const IEEE_BINARY64_KIND: &str = "math/ieee-binary64";
pub const IEEE_BINARY64_QUIET_KIND: &str = "math/ieee-binary64-compare-quiet";
pub const IEEE_BINARY64_RESULT_MAX_BYTES: u32 = 8192;
pub const IEEE_BINARY64_CONFIG_MAX_BYTES: u32 = 1024;

pub fn ieee_binary64_result_type() -> StructuredInfoType {
    named("IeeeBinary64Result")
}
pub fn ieee_binary64_comparison_type() -> StructuredInfoType {
    named("IeeeBinary64ComparisonResult")
}

fn fields(quiet: bool) -> Vec<(&'static str, &'static str, &'static str)> {
    if quiet {
        vec![("predicate", "IeeeBinary64QuietPredicate", "equal")]
    } else {
        vec![
            ("operation", "IeeeBinary64Operation", "add"),
            ("rounding", "NumericalRounding", "nearest_ties_even"),
        ]
    }
}
fn default_configuration(ty: StructuredInfoType, tag: &str) -> StructuredConfigurationValue {
    let value = StructuredInfoValue::variant(
        ty.clone(),
        tag,
        StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
            vec![],
        )
        .unwrap(),
    )
    .unwrap();
    StructuredConfigurationValue::new(
        ty.profile().unwrap().value_kind().clone(),
        value.canonical_bytes().unwrap(),
    )
    .unwrap()
}
fn contract(quiet: bool) -> Kind {
    let types = super::types();
    let named = |name: &str| {
        types
            .iter()
            .find(|ty| ty.name == name)
            .expect("reviewed IEEE Type")
            .value_type
            .clone()
    };
    let fields = fields(quiet);
    let result = if quiet {
        named("IeeeBinary64ComparisonResult")
    } else {
        named("IeeeBinary64Result")
    };
    let profile = result.profile().unwrap().value_kind().clone();
    let inputs = ["left", "right"]
        .map(|name| PortDescriptor {
            port_id: port_id(name),
            value_kind: kind_id(F64_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        })
        .to_vec();
    let mut values = vec![];
    for name in ["left", "right"] {
        values.push(FrontValueContract {
            location: FrontValueLocation::Input(port_id(name)),
            contract: CheckedValueContract::new(kind_id(F64_INFO_ID), 8, vec![]).unwrap(),
        });
    }
    values.push(FrontValueContract {
        location: FrontValueLocation::Output(port_id("result")),
        contract: CheckedValueContract::new(
            profile.clone(),
            IEEE_BINARY64_RESULT_MAX_BYTES,
            vec![],
        )
        .unwrap(),
    });
    for (name, ty, _) in &fields {
        values.push(FrontValueContract {
            location: FrontValueLocation::Startup((*name).into()),
            contract: CheckedValueContract::new(
                named(ty).profile().unwrap().value_kind().clone(),
                IEEE_BINARY64_CONFIG_MAX_BYTES,
                vec![],
            )
            .unwrap(),
        });
    }
    Kind {
        kind_id: kind_id(if quiet {
            IEEE_BINARY64_QUIET_KIND
        } else {
            IEEE_BINARY64_KIND
        }),
        kind_contract_revision: if quiet {
            "conduit.numeric/ieee-binary64-quiet@1"
        } else {
            "conduit.numeric/ieee-binary64@1"
        }
        .into(),
        startup_parameters: fields
            .iter()
            .map(|(name, ty, _)| FrontStartupParameter {
                name: (*name).into(),
                value_type: named(ty).profile().unwrap().value_kind().clone(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        inputs,
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: profile,
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: fields
            .iter()
            .map(|(name, ty, tag)| {
                let value = default_configuration(named(ty), tag);
                KindConfigurationField {
                    key: (*name).into(),
                    rule: KindConfigurationRule::Structured {
                        profile: value.profile().clone(),
                    },
                    default_value: ConfigurationValue::Structured(value),
                }
            })
            .collect(),
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce),
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
            KindSemanticLaw::ValueContracts(values),
        ],
        limits: CapabilityLimits {
            max_active_instances: 32,
            max_queue_items: 1,
            max_queue_bytes: IEEE_BINARY64_RESULT_MAX_BYTES,
        },
    }
}
pub fn ieee_binary64_semantic_contract() -> Kind {
    contract(false)
}
pub fn ieee_binary64_quiet_semantic_contract() -> Kind {
    contract(true)
}

pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for quiet in [false, true] {
        let kind = contract(quiet);
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: fields(quiet)
                    .iter()
                    .map(|(name, ty, _)| StartupParameterSignature {
                        name: (*name).into(),
                        value_type: (*ty).into(),
                        default: None,
                    })
                    .collect(),
            })
            .map_err(|why| format!("IEEE signature: {why:?}"))?;
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .map_err(|why| format!("IEEE Fore: {why:?}"))?;
        profile
            .insert_kind(kind)
            .map_err(|why| format!("IEEE Kind: {why:?}"))?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IeeeBinary64Configuration {
    Arithmetic {
        operation: IeeeBinary64Operation,
        rounding: Binary64Rounding,
    },
    Quiet {
        predicate: IeeeBinary64QuietPredicate,
    },
}
fn tag(entries: &[ConfigurationEntry], key: &str, ty_name: &str) -> Result<String, String> {
    let Some(ConfigurationValue::Structured(value)) =
        entries.iter().find(|e| e.key == key).map(|e| &e.value)
    else {
        return Err(format!("ieee/configuration/missing-{key}@1"));
    };
    let ty = named(ty_name);
    if value.profile() != ty.profile().unwrap().value_kind()
        || value.canonical_value().len() > IEEE_BINARY64_CONFIG_MAX_BYTES as usize
    {
        return Err("ieee/configuration/foreign-type-or-extent@1".into());
    }
    let decoded = StructuredInfoValue::from_canonical_bytes(value.canonical_value())
        .map_err(|_| "ieee/configuration/malformed@1")?;
    if decoded.value_type() != &ty {
        return Err("ieee/configuration/foreign-type@1".into());
    }
    let StructuredInfoValueShape::Variant { tag, .. } = decoded.shape() else {
        return Err("ieee/configuration/expected-case@1".into());
    };
    Ok(tag.into())
}
/// Decode exact typed startup values at preparation/inspection boundaries.
/// The caller separately verifies the Kind, Fore and selected Back identity.
pub fn ieee_binary64_configuration(
    kind: &KindId,
    entries: &[ConfigurationEntry],
) -> Result<IeeeBinary64Configuration, String> {
    let invalid = || String::from("ieee/configuration/unsupported-case@1");
    if kind.as_str() == IEEE_BINARY64_KIND && entries.len() == 2 {
        let operation = match tag(entries, "operation", "IeeeBinary64Operation")?.as_str() {
            "add" => IeeeBinary64Operation::Add,
            "subtract" => IeeeBinary64Operation::Subtract,
            "multiply" => IeeeBinary64Operation::Multiply,
            "divide" => IeeeBinary64Operation::Divide,
            _ => return Err(invalid()),
        };
        let rounding = match tag(entries, "rounding", "NumericalRounding")?.as_str() {
            "nearest_ties_even" => Binary64Rounding::NearestTiesEven,
            "toward_zero" => Binary64Rounding::TowardZero,
            "toward_positive" => Binary64Rounding::TowardPositive,
            "toward_negative" => Binary64Rounding::TowardNegative,
            _ => return Err(invalid()),
        };
        Ok(IeeeBinary64Configuration::Arithmetic {
            operation,
            rounding,
        })
    } else if kind.as_str() == IEEE_BINARY64_QUIET_KIND && entries.len() == 1 {
        let predicate = match tag(entries, "predicate", "IeeeBinary64QuietPredicate")?.as_str() {
            "equal" => IeeeBinary64QuietPredicate::Equal,
            "less" => IeeeBinary64QuietPredicate::Less,
            "less_equal" => IeeeBinary64QuietPredicate::LessEqual,
            "unordered" => IeeeBinary64QuietPredicate::Unordered,
            _ => return Err(invalid()),
        };
        Ok(IeeeBinary64Configuration::Quiet { predicate })
    } else {
        Err("ieee/configuration/wrong-kind-or-fields@1".into())
    }
}
