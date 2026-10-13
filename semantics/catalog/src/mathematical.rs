//! Checked mathematical signatures; these contracts promise no numerical algorithm.
use alloc::{format, vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ExternalEffectBehavior, Kind, KindIdentity,
    KindSemanticLaw, PortDescriptor, PortDirection, PortTemporal, ReplayBehavior,
    SemanticDependence, SuspensionBehavior, TemporalStateBehavior, VariabilityBehavior,
    ANGLE_INFO_ID, MATH_VALUE_INFO_ID, QUANTITY_INFO_ID,
};

pub const MATHEMATICAL_CONTRACT_REVISION: &str = "conduit.math/exact-denotation@1";
pub const MATHEMATICAL_KINDS: &[&str] = &[
    "math/sin",
    "math/cos",
    "math/tan",
    "math/asin",
    "math/acos",
    "math/atan",
    "math/atan2",
    "math/sqrt",
    "math/root",
    "math/pow",
    "math/cyclic-to-angular-frequency",
    "math/angular-to-cyclic-frequency",
];

pub fn mathematical_kind(name: &str) -> Option<Kind> {
    let (inputs, output): (&[(&str, &str)], &str) = match name {
        "math/sin" | "math/cos" | "math/tan" => (&[("angle", ANGLE_INFO_ID)], MATH_VALUE_INFO_ID),
        "math/asin" | "math/acos" | "math/atan" => {
            (&[("value", MATH_VALUE_INFO_ID)], ANGLE_INFO_ID)
        }
        "math/sqrt" => (&[("value", MATH_VALUE_INFO_ID)], MATH_VALUE_INFO_ID),
        "math/pow" => (
            &[
                ("base", MATH_VALUE_INFO_ID),
                ("exponent", MATH_VALUE_INFO_ID),
            ],
            MATH_VALUE_INFO_ID,
        ),
        "math/root" => (
            &[
                ("value", MATH_VALUE_INFO_ID),
                ("degree", MATH_VALUE_INFO_ID),
            ],
            MATH_VALUE_INFO_ID,
        ),
        "math/cyclic-to-angular-frequency" => (&[("frequency", conduit_core::FREQUENCY_INFO_ID)], conduit_core::ANGULAR_FREQUENCY_INFO_ID),
        "math/angular-to-cyclic-frequency" => (&[("frequency", conduit_core::ANGULAR_FREQUENCY_INFO_ID)], conduit_core::FREQUENCY_INFO_ID),
        "math/atan2" => (
            &[("y", QUANTITY_INFO_ID), ("x", QUANTITY_INFO_ID)],
            ANGLE_INFO_ID,
        ),
        _ => return None,
    };
    Some(Kind {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!(
            "{MATHEMATICAL_CONTRACT_REVISION}/{name}"
        )),
        startup_parameters: vec![],
        shorthand: (inputs.len() == 1).then(|| (port_id(inputs[0].0), port_id("result"))),
        inputs: inputs
            .iter()
            .map(|(name, kind)| port(name, kind, PortDirection::Input))
            .collect(),
        outputs: vec![port("result", output, PortDirection::Output)],
        configuration: vec![],
        semantic_laws: vec![
            KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
            KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
            KindSemanticLaw::TimeDependence(SemanticDependence::None),
            KindSemanticLaw::RandomDependence(SemanticDependence::None),
            KindSemanticLaw::ResourceDependence(SemanticDependence::None),
            KindSemanticLaw::Suspension(SuspensionBehavior::Never),
            KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
            KindSemanticLaw::Replay(ReplayBehavior::Exact),
        ],
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::MAX_NUMERIC_ENCODED_BYTES as u32,
        },
    })
}
fn port(name: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_mathematical_kinds(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    for name in MATHEMATICAL_KINDS {
        let kind = mathematical_kind(name).expect("reviewed mathematical Kind");
        startup.insert(conduit_plot::KindSignature {
            kind: (*name).into(),
            startup_parameters: vec![],
        })?;
        startup.insert_fore(name, kind.checked_front())?;
        profile
            .insert_kind(kind)
            .map_err(|reason| reason.to_string())?;
    }
    install_evaluate(startup, profile)?;
    crate::numeric_tensor::install_tensor_evaluation(startup, profile)
}

/// Source facade; expansion specializes its result and seals each consumer graph.
#[cfg(feature = "plot-catalog")]
fn install_evaluate(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use alloc::string::ToString;
    startup.insert_structured_type(
        "NumericRequirement",
        conduit_core::numeric_requirement_info_type(),
    )?;
    startup.insert(conduit_plot::KindSignature {
        kind: "math/evaluate".into(),
        startup_parameters: vec![
            conduit_plot::StartupParameterSignature {
                name: "value".into(),
                value_type: "Scalar".into(),
                default: None,
            },
            conduit_plot::StartupParameterSignature {
                name: "requirement".into(),
                value_type: "NumericRequirement".into(),
                default: None,
            },
        ],
    })?;
    profile
        .insert(conduit_plot::KindProjection {
            kind_id: kind_id("math/evaluate"),
            kind_contract_revision: KindIdentity::from("conduit.math/evaluate-source@1"),
            inputs: vec![],
            outputs: vec![port("result", MATH_VALUE_INFO_ID, PortDirection::Output)],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}
