//! Portable numerical meaning, authored Types and immutable realization receipts.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::{ProfileCatalog, StartupCatalog};
mod ieee;
mod near;
pub use ieee::*;
pub use near::*;
pub const NUMERICAL_FACE_KIND: &str = "face/numerical-result";
pub const NUMERICAL_REALIZE_KIND: &str = "math/realize";
pub const NUMERICAL_REALIZE_REVISION: &str = "conduit.numeric/realize@1";
pub const NUMERICAL_REQUEST_MAX_BYTES: u32 = 8192;
pub const NUMERICAL_RESULT_MAX_BYTES: u32 = 8192;
pub const NUMERICAL_TYPES_SOURCE: &str = include_str!("../numerical/types.conduit");
fn types() -> Vec<conduit_plot::CheckedNativeType> {
    conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(NUMERICAL_TYPES_SOURCE),
        &StartupCatalog::new(),
    )
    .expect("reviewed finite numerical Types")
    .native_types
}
fn named(name: &str) -> StructuredInfoType {
    types()
        .into_iter()
        .find(|ty| ty.name == name)
        .expect("reviewed numerical Type name")
        .value_type
}
pub fn numerical_request_type() -> StructuredInfoType {
    named("NumericalRequest")
}
pub fn numerical_result_type() -> StructuredInfoType {
    named("NumericalResult")
}
fn math(ty: StructuredInfoType, value: &MathematicalValue) -> Result<StructuredInfoValue, String> {
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let length = value
        .encode(&mut bytes)
        .map_err(|why| format!("numeric value: {why:?}"))?;
    bytes.truncate(length);
    StructuredInfoValue::leaf(ty, bytes).map_err(|why| format!("numeric leaf: {why:?}"))
}
pub fn numerical_request_value(
    source: &MathematicalValue,
    error: ExactRational,
) -> Result<StructuredInfoValue, String> {
    let ty = numerical_request_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let values = vec![
        StructuredFieldValue::new("value", math(fields[0].value_type().clone(), source)?)
            .map_err(|why| format!("numeric request: {why:?}"))?,
        StructuredFieldValue::new(
            "maximum-error",
            math(
                fields[1].value_type().clone(),
                &MathematicalValue::rational(error),
            )?,
        )
        .map_err(|why| format!("numeric error: {why:?}"))?,
    ];
    StructuredInfoValue::record(ty, values).map_err(|why| format!("numeric request: {why:?}"))
}
pub fn numerical_realize_semantic_contract() -> Kind {
    let request = numerical_request_type();
    let request_profile = request.profile().unwrap().value_kind().clone();
    let default = numerical_request_value(
        &MathematicalValue::rational(ExactRational::ZERO),
        ExactRational::ZERO,
    )
    .unwrap();
    Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "request".into(),
            value_type: request_profile.clone(),
            has_default: false,
        }],
        shorthand: None,
        kind_id: kind_id(NUMERICAL_REALIZE_KIND),
        kind_contract_revision: NUMERICAL_REALIZE_REVISION.into(),
        inputs: vec![],
        outputs: vec![PortDescriptor {
            port_id: port_id("result"),
            value_kind: numerical_result_type()
                .profile()
                .unwrap()
                .value_kind()
                .clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
            abnormal_kind: None,
        }],
        configuration: vec![KindConfigurationField {
            key: "request".into(),
            default_value: ConfigurationValue::Structured(
                StructuredConfigurationValue::new(
                    request_profile.clone(),
                    default.canonical_bytes().unwrap(),
                )
                .unwrap(),
            ),
            rule: KindConfigurationRule::Structured {
                profile: request_profile,
            },
        }],
        semantic_laws: vec![
            KindSemanticLaw::Terminal(KindTerminalBehavior::EmitsOnce),
            KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Startup("request".into()),
                    contract: CheckedValueContract::new(
                        request.profile().unwrap().value_kind().clone(),
                        NUMERICAL_REQUEST_MAX_BYTES,
                        vec![],
                    )
                    .unwrap(),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("result")),
                    contract: CheckedValueContract::new(
                        numerical_result_type()
                            .profile()
                            .unwrap()
                            .value_kind()
                            .clone(),
                        NUMERICAL_RESULT_MAX_BYTES,
                        vec![],
                    )
                    .unwrap(),
                },
            ]),
            KindSemanticLaw::RealizationRequirement {
                property_profile: numerical_realization_profile(),
                configuration_key: "request".into(),
            },
        ],
        limits: CapabilityLimits {
            max_active_instances: 32,
            max_queue_items: 1,
            max_queue_bytes: NUMERICAL_RESULT_MAX_BYTES,
        },
    }
}
pub fn numerical_face_semantic_contract() -> Kind {
    let mut kind =
        crate::structured_face_semantic_contract("NumericalResult", &numerical_result_type());
    kind.kind_id = kind_id(NUMERICAL_FACE_KIND);
    kind.kind_contract_revision = "numeric-result/face@1".into();
    kind
}
pub fn install_numerical_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    for ty in types() {
        startup
            .insert_structured_type(&ty.name, ty.value_type)
            .map_err(|why| format!("numeric Type: {why:?}"))?;
    }
    let presenter = numerical_face_semantic_contract();
    startup
        .insert(conduit_plot::KindSignature {
            kind: NUMERICAL_FACE_KIND.into(),
            startup_parameters: crate::data_label_startup_signature(),
        })
        .map_err(|why| format!("numeric Face: {why:?}"))?;
    startup
        .insert_fore(NUMERICAL_FACE_KIND, presenter.checked_front())
        .map_err(|why| format!("numeric Face Fore: {why:?}"))?;
    profile
        .insert_kind(presenter)
        .map_err(|why| format!("numeric Face Kind: {why:?}"))?;
    let kind = numerical_realize_semantic_contract();
    startup
        .insert(conduit_plot::KindSignature {
            kind: NUMERICAL_REALIZE_KIND.into(),
            startup_parameters: vec![conduit_plot::StartupParameterSignature {
                name: "request".into(),
                value_type: "NumericalRequest".into(),
                default: None,
            }],
        })
        .map_err(|why| format!("numeric signature: {why:?}"))?;
    startup
        .insert_fore(NUMERICAL_REALIZE_KIND, kind.checked_front())
        .map_err(|why| format!("numeric Fore: {why:?}"))?;
    profile
        .insert_kind(kind)
        .map_err(|why| format!("numeric Kind: {why:?}"))?;
    near::install(startup, profile)?;
    ieee::install(startup, profile)
}
pub fn numerical_result_value(
    prepared: &PreparedNumericalRealization,
) -> Result<StructuredInfoValue, String> {
    let ty = numerical_result_type();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        unreachable!()
    };
    let (lower, upper, tag) = match prepared.target() {
        NumericalTarget::ExactRational(q) => (**q, **q, "exact_rational"),
        NumericalTarget::Binary64(algorithm) => {
            let q = ExactRational::from_binary64(
                algorithm.target(),
                &mut ExactNumericWork::new(100_000_000),
            )
            .map_err(|why| format!("binary64 receipt denotation: {why:?}"))?;
            (q, q, "binary64")
        }
        NumericalTarget::EnclosedBinary64(algorithm) => {
            let q = ExactRational::from_binary64(
                algorithm.rounded().target(),
                &mut ExactNumericWork::new(100_000_000),
            )
            .map_err(|why| format!("enclosed binary64 receipt: {why:?}"))?;
            (q, q, "binary64")
        }
        NumericalTarget::OutwardI64(interval) => (
            ExactRational::from_i128(
                interval
                    .lower()
                    .signed()
                    .map_err(|why| format!("numeric endpoint: {why:?}"))?,
            ),
            ExactRational::from_i128(
                interval
                    .upper()
                    .signed()
                    .map_err(|why| format!("numeric endpoint: {why:?}"))?,
            ),
            "outward_i64",
        ),
    };
    let mut values = Vec::new();
    for (name, value) in [
        ("source", prepared.source().clone()),
        ("lower", MathematicalValue::rational(lower)),
        ("upper", MathematicalValue::rational(upper)),
        (
            "maximum-error",
            MathematicalValue::rational(prepared.maximum_error()),
        ),
    ] {
        let field = fields.iter().find(|field| field.name() == name).unwrap();
        values.push(
            StructuredFieldValue::new(name, math(field.value_type().clone(), &value)?)
                .map_err(|why| format!("numeric result field: {why:?}"))?,
        );
    }
    let field = fields
        .iter()
        .find(|field| field.name() == "representation")
        .unwrap();
    let payload = match prepared.target() {
        NumericalTarget::Binary64(algorithm) => {
            binary64_receipt_payload(field.value_type(), algorithm, false)?
        }
        NumericalTarget::EnclosedBinary64(algorithm) => {
            binary64_receipt_payload(field.value_type(), algorithm.rounded(), true)?
        }
        _ => StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
            vec![],
        )
        .unwrap(),
    };
    values.push(
        StructuredFieldValue::new(
            "representation",
            StructuredInfoValue::variant(field.value_type().clone(), tag, payload)
                .map_err(|why| format!("numeric representation: {why:?}"))?,
        )
        .map_err(|why| format!("numeric result: {why:?}"))?,
    );
    StructuredInfoValue::record(ty, values).map_err(|why| format!("numeric result: {why:?}"))
}
fn binary64_receipt_payload(
    ty: &StructuredInfoType,
    algorithm: &PreparedBinary64Algorithm,
    certified_enclosure: bool,
) -> Result<StructuredInfoValue, String> {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        return Err("numeric representation Type".into());
    };
    let ty = cases
        .iter()
        .find(|case| case.tag() == "binary64")
        .ok_or("binary64 representation case")?
        .payload_type()
        .clone();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        return Err("binary64 representation payload".into());
    };
    let leaf = |name: &str, bytes: Vec<u8>| -> Result<StructuredFieldValue, String> {
        let field = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or("binary64 receipt field")?;
        let value = StructuredInfoValue::leaf(field.value_type().clone(), bytes)
            .map_err(|why| format!("binary64 receipt leaf: {why:?}"))?;
        StructuredFieldValue::new(name, value)
            .map_err(|why| format!("binary64 receipt field: {why:?}"))
    };
    let tag = |name: &str, tag: &str| -> Result<StructuredFieldValue, String> {
        let field = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or("binary64 receipt tag")?;
        let empty = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID)).unwrap(),
            vec![],
        )
        .unwrap();
        let value = StructuredInfoValue::variant(field.value_type().clone(), tag, empty)
            .map_err(|why| format!("binary64 receipt tag: {why:?}"))?;
        StructuredFieldValue::new(name, value)
            .map_err(|why| format!("binary64 receipt field: {why:?}"))
    };
    let values = vec![
        leaf("target", algorithm.target().encode().to_vec())?,
        tag(
            "rounding",
            match algorithm.rounding() {
                Binary64Rounding::NearestTiesEven => "nearest_ties_even",
                Binary64Rounding::TowardZero => "toward_zero",
                Binary64Rounding::TowardPositive => "toward_positive",
                Binary64Rounding::TowardNegative => "toward_negative",
            },
        )?,
        leaf(
            "any-inexact",
            vec![u8::from(
                algorithm
                    .steps()
                    .iter()
                    .any(|s| s.projection().status().inexact),
            )],
        )?,
        leaf(
            "any-tiny-inexact",
            vec![u8::from(
                algorithm
                    .steps()
                    .iter()
                    .any(|s| s.projection().status().tiny_inexact),
            )],
        )?,
        leaf("step-count", vec![algorithm.steps().len() as u8])?,
        tag(
            "error-claim",
            if certified_enclosure {
                "certified_bound"
            } else {
                match algorithm.error() {
                    Binary64ErrorClaim::ExactDistance(_) => "exact_distance",
                    Binary64ErrorClaim::CertifiedBound(_) => "certified_bound",
                }
            },
        )?,
    ];
    StructuredInfoValue::record(ty, values).map_err(|why| format!("binary64 receipt: {why:?}"))
}
