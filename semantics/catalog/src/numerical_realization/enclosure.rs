//! Exact input data and a bounded rational interval, using existing projection laws.
use super::*;
pub const MATHEMATICAL_ENCLOSE_KIND: &str = "math/enclose";
pub const MATHEMATICAL_ENCLOSE_REVISION: &str = "numeric/mathematical-enclosure@1";
pub const MATHEMATICAL_ENCLOSE_MAX_BYTES: u32 = 65536;
pub const MATHEMATICAL_ENCLOSE_REQUEST_MAX_BYTES: u32 = 4096;
pub fn mathematical_enclosure_request_type() -> StructuredInfoType {
    super::named("MathematicalEnclosureRequest")
}
pub fn mathematical_enclosure_type() -> StructuredInfoType {
    super::named("MathematicalEnclosure")
}
fn default_request() -> StructuredConfigurationValue {
    let ty = mathematical_enclosure_request_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "maximum-width",
            super::math(
                fields[0].value_type().clone(),
                &MathematicalValue::rational(ExactRational::ZERO),
            )
            .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    StructuredConfigurationValue::new(
        ty.profile().unwrap().value_kind().clone(),
        value.canonical_bytes().unwrap(),
    )
    .unwrap()
}
pub fn mathematical_enclose_semantic_contract() -> Kind {
    let request = mathematical_enclosure_request_type();
    let request_profile = request.profile().unwrap().value_kind().clone();
    let result = mathematical_enclosure_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    Kind {
        kind_id: kind_id(MATHEMATICAL_ENCLOSE_KIND),
        kind_contract_revision: MATHEMATICAL_ENCLOSE_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "request".into(),
            value_type: request_profile.clone(),
            has_default: false,
        }],
        shorthand: Some((port_id("value"), port_id("result"))),
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(MATHEMATICAL_VALUE_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: result.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "request".into(),
            default_value: ConfigurationValue::Structured(default_request()),
            rule: KindConfigurationRule::Structured {
                profile: request_profile.clone(),
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                property_profile: request_profile.clone(),
                configuration_key: "request".into(),
            }),
            KindSemanticLaw::Terminal(
                KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
            ),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Startup("request".into()),
                    contract: CheckedValueContract::new(
                        request_profile,
                        MATHEMATICAL_ENCLOSE_REQUEST_MAX_BYTES,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("value")),
                    contract: CheckedValueContract::new(
                        kind_id(MATHEMATICAL_VALUE_ID),
                        MATHEMATICAL_VALUE_MAX_ENCODED_BYTES as u32,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("result")),
                    contract: CheckedValueContract::new(
                        result,
                        MATHEMATICAL_ENCLOSE_MAX_BYTES,
                        vec![],
                    )
                    .unwrap(),
                },
            ]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 32,
            max_queue_items: 1,
            max_queue_bytes: MATHEMATICAL_ENCLOSE_MAX_BYTES,
        },
    }
}
/// Admit a rational, nonnegative width from the exact checked request. This is
/// preparation work; the returned requirement does not attest an evaluated input.
pub fn mathematical_enclose_requirements(
    configuration: &[ConfigurationEntry],
) -> Result<MathematicalEnclosureRequirements, String> {
    let [entry] = configuration else {
        return Err("numeric/enclose-one-request-required@1".into());
    };
    let ("request", ConfigurationValue::Structured(request)) = (entry.key.as_str(), &entry.value)
    else {
        return Err("numeric/enclose-typed-request-required@1".into());
    };
    if request.canonical_value().len() > MATHEMATICAL_ENCLOSE_REQUEST_MAX_BYTES as usize
        || request.profile()
            != mathematical_enclosure_request_type()
                .profile()
                .unwrap()
                .value_kind()
    {
        return Err("numeric/enclose-request-profile-or-extent@1".into());
    }
    let value = StructuredInfoValue::from_canonical_bytes(request.canonical_value())
        .map_err(|why| format!("numeric/enclose-request-invalid@1: {why:?}"))?;
    if value.value_type() != &mathematical_enclosure_request_type() {
        return Err("numeric/enclose-request-type@1".into());
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        unreachable!()
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
        unreachable!()
    };
    let mut work = ExactNumericWork::new(100_000_000);
    let maximum_width = MathematicalValue::decode(bytes, &mut work)
        .map_err(|why| format!("numeric/enclose-width-invalid@1: {why:?}"))?
        .as_rational()
        .ok_or("numeric/enclose-rational-width-required@1")?;
    if maximum_width.is_negative() {
        return Err("numeric/enclose-negative-width@1".into());
    }
    let maximum_leaf_width = maximum_width
        .checked_divide(ExactRational::from_i128(64), &mut work)
        .map_err(|why| format!("numeric/enclose-width-profile@1: {why:?}"))?;
    Ok(MathematicalEnclosureRequirements {
        maximum_width,
        maximum_leaf_width,
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    })
}
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let kind = mathematical_enclose_semantic_contract();
    startup
        .insert(conduit_plot::KindSignature {
            kind: MATHEMATICAL_ENCLOSE_KIND.into(),
            startup_parameters: vec![conduit_plot::StartupParameterSignature {
                name: "request".into(),
                value_type: "MathematicalEnclosureRequest".into(),
                default: None,
            }],
        })
        .map_err(|why| format!("enclose signature: {why:?}"))?;
    startup
        .insert_fore(MATHEMATICAL_ENCLOSE_KIND, kind.checked_front())
        .map_err(|why| format!("enclose Fore: {why:?}"))?;
    profile
        .insert_kind(kind)
        .map_err(|why| format!("enclose Kind: {why:?}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "enclosure_tests.rs"]
mod tests;
