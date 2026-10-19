//! Exact coordinate projection of actual input data, independent of rounding.
use super::*;
pub const QUANTITY_COORDINATES_KIND: &str = "math/coordinates";
pub const QUANTITY_COORDINATES_REVISION: &str = "physical/quantity-coordinates@1";
pub const QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES: u32 = 131500;
pub const QUANTITY_COORDINATES_REQUEST_MAX_BYTES: u32 = 4096;
pub fn quantity_coordinate_request_type() -> StructuredInfoType {
    super::named("QuantityCoordinateRequest")
}
pub fn quantity_coordinate_evidence_type() -> StructuredInfoType {
    super::named("QuantityCoordinateEvidence")
}
fn default_request() -> StructuredConfigurationValue {
    let ty = quantity_coordinate_request_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "unit",
            StructuredInfoValue::leaf(
                fields[0].value_type().clone(),
                Unit::Meter.encode().to_vec(),
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
pub fn quantity_coordinates_semantic_contract() -> Kind {
    let request = quantity_coordinate_request_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let evidence = quantity_coordinate_evidence_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let port = |name: &str, kind: KindId, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind,
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let contract = |location, kind, bound| FrontValueContract {
        location,
        contract: CheckedValueContract::new(kind, bound, vec![]).unwrap(),
    };
    Kind {
        kind_id: kind_id(QUANTITY_COORDINATES_KIND),
        kind_contract_revision: QUANTITY_COORDINATES_REVISION.into(),
        startup_parameters: vec![FrontStartupParameter {
            name: "request".into(),
            value_type: request.clone(),
            has_default: false,
        }],
        shorthand: None,
        inputs: vec![port(
            "value",
            kind_id(QUANTITY_INFO_ID),
            PortDirection::Input,
        )],
        outputs: vec![
            port("result", kind_id(QUANTITY_INFO_ID), PortDirection::Output),
            port("evidence", evidence.clone(), PortDirection::Output),
        ],
        configuration: vec![KindConfigurationField {
            key: "request".into(),
            default_value: ConfigurationValue::Structured(default_request()),
            rule: KindConfigurationRule::Structured {
                profile: request.clone(),
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::ExactRealizationProperty(ExactRealizationProperty {
                property_profile: request.clone(),
                configuration_key: "request".into(),
            }),
            KindSemanticLaw::Terminal(
                KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
            ),
            KindSemanticLaw::ValueContracts(vec![
                contract(
                    FrontValueLocation::Startup("request".into()),
                    request,
                    QUANTITY_COORDINATES_REQUEST_MAX_BYTES,
                ),
                contract(
                    FrontValueLocation::Input(port_id("value")),
                    kind_id(QUANTITY_INFO_ID),
                    MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
                ),
                contract(
                    FrontValueLocation::Output(port_id("result")),
                    kind_id(QUANTITY_INFO_ID),
                    MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES as u32,
                ),
                contract(
                    FrontValueLocation::Output(port_id("evidence")),
                    evidence,
                    QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
                ),
            ]),
        ],
        limits: CapabilityLimits {
            max_active_instances: 32,
            max_queue_items: 1,
            max_queue_bytes: QUANTITY_COORDINATES_EVIDENCE_MAX_BYTES,
        },
    }
}
/// Resolve only the already-bound Unit capsule; no new definition authority.
pub fn quantity_coordinate_target(configuration: &[ConfigurationEntry]) -> Result<Unit, String> {
    let [entry] = configuration else {
        return Err("coordinates/one-request-required@1".into());
    };
    let ("request", ConfigurationValue::Structured(request)) = (entry.key.as_str(), &entry.value)
    else {
        return Err("coordinates/typed-request-required@1".into());
    };
    if request.canonical_value().len() > QUANTITY_COORDINATES_REQUEST_MAX_BYTES as usize
        || request.profile()
            != quantity_coordinate_request_type()
                .profile()
                .unwrap()
                .value_kind()
    {
        return Err("coordinates/request-profile-or-extent@1".into());
    }
    let value = StructuredInfoValue::from_canonical_bytes(request.canonical_value())
        .map_err(|why| format!("coordinates/request-invalid@1: {why:?}"))?;
    if value.value_type() != &quantity_coordinate_request_type() {
        return Err("coordinates/request-type@1".into());
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        unreachable!()
    };
    let StructuredInfoValueShape::Leaf(bytes) = fields[0].value().shape() else {
        unreachable!()
    };
    Unit::decode(bytes).map_err(|why| format!("coordinates/unit-invalid@1: {why:?}"))
}
pub(super) fn install(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    let kind = quantity_coordinates_semantic_contract();
    startup
        .insert(conduit_plot::KindSignature {
            kind: QUANTITY_COORDINATES_KIND.into(),
            startup_parameters: vec![conduit_plot::StartupParameterSignature {
                name: "request".into(),
                value_type: "QuantityCoordinateRequest".into(),
                default: None,
            }],
        })
        .map_err(|why| format!("coordinates signature: {why:?}"))?;
    startup
        .insert_fore(QUANTITY_COORDINATES_KIND, kind.checked_front())
        .map_err(|why| format!("coordinates Fore: {why:?}"))?;
    profile
        .insert_kind(kind)
        .map_err(|why| format!("coordinates Kind: {why:?}"))?;
    Ok(())
}

#[cfg(test)]
#[path = "coordinates_tests.rs"]
mod tests;
