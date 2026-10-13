//! Exact bounded scalar-control contracts and no-std semantic functions.

use super::{
    KindConfigurationField, KindConfigurationRule, KindTerminalBehavior, StandardKindContract,
};
#[cfg(feature = "plot-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, ExternalEffectBehavior, Kind,
    KindSemanticLaw, NumericRefusal, PortDescriptor, PortDirection, PortTemporal,
    PortableNumericProfile, ReplayBehavior, Scalar, SemanticDependence, SuspensionBehavior,
    TemporalStateBehavior, VariabilityBehavior, MAX_NUMERIC_ENCODED_BYTES, SCALAR_INFO_ID,
};
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MathScalarRefusal {
    InvalidConfiguration,
    Arithmetic(NumericRefusal),
    UnresolvedComparison,
}

pub const MATH_CLAMP_KIND: &str = "math/clamp";
pub const MATH_SCALE_KIND: &str = "math/scale";
pub const MATH_DEADBAND_KIND: &str = "math/deadband";

pub const MATH_CLAMP_CONTRACT_REVISION: &str = "conduit.std/math-clamp-scalar@1";
pub const MATH_SCALE_CONTRACT_REVISION: &str = "conduit.std/math-scale-scalar@1";
pub const MATH_DEADBAND_CONTRACT_REVISION: &str = "conduit.std/math-deadband-scalar@1";

pub const SCALAR_INPUT_PORT: &str = "in";
pub const SCALAR_OUTPUT_PORT: &str = "out";
pub const CLAMP_MINIMUM_KEY: &str = "minimum";
pub const CLAMP_MAXIMUM_KEY: &str = "maximum";
pub const SCALE_GAIN_KEY: &str = "gain";
pub const DEADBAND_RADIUS_KEY: &str = "radius";

pub const DEFAULT_CLAMP_MINIMUM: Scalar = Scalar::integer(-1);
pub const DEFAULT_CLAMP_MAXIMUM: Scalar = Scalar::ONE;
pub const DEFAULT_SCALE_GAIN: Scalar = Scalar::ONE;

fn compare(left: &Scalar, right: &Scalar) -> Result<core::cmp::Ordering, MathScalarRefusal> {
    left.compare(right)
        .map_err(MathScalarRefusal::Arithmetic)?
        .ok_or(MathScalarRefusal::UnresolvedComparison)
}

pub fn clamp_scalar(
    input: Scalar,
    minimum: Scalar,
    maximum: Scalar,
) -> Result<Scalar, MathScalarRefusal> {
    use core::cmp::Ordering;
    if compare(&minimum, &maximum)? == Ordering::Greater {
        return Err(MathScalarRefusal::InvalidConfiguration);
    }
    if compare(&input, &minimum)? == Ordering::Less {
        return Ok(minimum);
    }
    if compare(&input, &maximum)? == Ordering::Greater {
        return Ok(maximum);
    }
    Ok(input)
}

pub fn scale_scalar(input: Scalar, gain: Scalar) -> Result<Scalar, MathScalarRefusal> {
    input
        .checked_mul(&gain)
        .map_err(MathScalarRefusal::Arithmetic)
}

pub fn deadband_scalar(input: Scalar, radius: Scalar) -> Result<Scalar, MathScalarRefusal> {
    use core::cmp::Ordering;
    if compare(&radius, &Scalar::ZERO)? == Ordering::Less {
        return Err(MathScalarRefusal::InvalidConfiguration);
    }
    let lower = radius
        .checked_neg()
        .map_err(MathScalarRefusal::Arithmetic)?;
    Ok(
        if compare(&input, &lower)? != Ordering::Less
            && compare(&input, &radius)? != Ordering::Greater
        {
            Scalar::ZERO
        } else {
            input
        },
    )
}

pub fn math_clamp_contract() -> StandardKindContract {
    contract(
        MATH_CLAMP_KIND,
        "Clamp scalar",
        "Limit one scalar to an inclusive configured minimum and maximum.",
        vec![
            scalar_field(CLAMP_MINIMUM_KEY, DEFAULT_CLAMP_MINIMUM),
            scalar_field(CLAMP_MAXIMUM_KEY, DEFAULT_CLAMP_MAXIMUM),
        ],
        "clamp: math/clamp(minimum = -1, maximum = 1)",
    )
}

pub fn math_scale_contract() -> StandardKindContract {
    contract(
        MATH_SCALE_KIND,
        "Scale scalar",
        "Multiply exact mathematical scalars with checked numeric bounds.",
        vec![scalar_field(SCALE_GAIN_KEY, DEFAULT_SCALE_GAIN)],
        "scale: math/scale(gain = 1)",
    )
}

pub fn math_deadband_contract() -> StandardKindContract {
    contract(
        MATH_DEADBAND_KIND,
        "Scalar deadband",
        "Emit zero inside an inclusive symmetric radius and preserve values outside it.",
        vec![scalar_field(
            DEADBAND_RADIUS_KEY,
            Scalar::from_decimal_parts(5, -2).expect("exact default radius"),
        )],
        "deadband: math/deadband(radius = 0.05)",
    )
}

pub fn math_clamp_semantic_contract() -> Kind {
    pure_semantic_contract(math_clamp_contract(), MATH_CLAMP_CONTRACT_REVISION)
}

pub fn math_scale_semantic_contract() -> Kind {
    pure_semantic_contract(math_scale_contract(), MATH_SCALE_CONTRACT_REVISION)
}

pub fn math_deadband_semantic_contract() -> Kind {
    pure_semantic_contract(math_deadband_contract(), MATH_DEADBAND_CONTRACT_REVISION)
}

fn pure_semantic_contract(contract: StandardKindContract, revision: &str) -> Kind {
    let mut contract = contract.into_semantic_contract(revision);
    contract.shorthand = Some((port_id(SCALAR_INPUT_PORT), port_id(SCALAR_OUTPUT_PORT)));
    contract.semantic_laws.extend([
        KindSemanticLaw::ExternalEffects(ExternalEffectBehavior::None),
        KindSemanticLaw::TemporalState(TemporalStateBehavior::None),
        KindSemanticLaw::TimeDependence(SemanticDependence::None),
        KindSemanticLaw::RandomDependence(SemanticDependence::None),
        KindSemanticLaw::ResourceDependence(SemanticDependence::None),
        KindSemanticLaw::Suspension(SuspensionBehavior::Never),
        KindSemanticLaw::Variability(VariabilityBehavior::DeterministicFromInputs),
        KindSemanticLaw::Replay(ReplayBehavior::Exact),
    ]);
    contract
}

fn contract(
    kind: &str,
    plain_name: &str,
    summary: &str,
    configuration: alloc::vec::Vec<KindConfigurationField>,
    example: &str,
) -> StandardKindContract {
    StandardKindContract {
        kind_id: kind_id(kind),
        plain_name: plain_name.to_string(),
        summary: summary.to_string(),
        inputs: vec![port(SCALAR_INPUT_PORT, PortDirection::Input)],
        outputs: vec![port(SCALAR_OUTPUT_PORT, PortDirection::Output)],
        configuration,
        limits: CapabilityLimits {
            max_active_instances: 16,
            max_queue_items: 1,
            max_queue_bytes: MAX_NUMERIC_ENCODED_BYTES as u32,
        },
        terminal_behavior:
            KindTerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

fn scalar_field(key: &str, default: Scalar) -> KindConfigurationField {
    KindConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::Scalar(default),
        rule: KindConfigurationRule::Scalar {
            profile: PortableNumericProfile::MAXIMAL,
        },
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[cfg(feature = "plot-catalog")]
pub fn install_math_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};
    for contract in [
        math_clamp_semantic_contract(),
        math_scale_semantic_contract(),
        math_deadband_semantic_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Scalar".to_string(),
                    default: Some(match &field.default_value {
                        ConfigurationValue::Scalar(value) => {
                            value.source_expression().expect("bounded Scalar default")
                        }
                        _ => unreachable!("math configuration is an exact Scalar"),
                    }),
                })
                .collect(),
        })?;
        profile
            .insert_kind(contract)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_scalar_algorithms_do_not_truncate_to_micro_grid() {
        let third = Scalar::rational("1", "3").unwrap();
        assert_eq!(
            scale_scalar(third.clone(), Scalar::integer(3)),
            Ok(Scalar::ONE)
        );
        let tiny = Scalar::parse_decimal("0.0000001").unwrap();
        assert_eq!(scale_scalar(tiny.clone(), Scalar::ONE), Ok(tiny));
        assert_eq!(
            clamp_scalar(third.clone(), Scalar::ZERO, Scalar::ONE),
            Ok(third)
        );
        assert_eq!(
            clamp_scalar(Scalar::ZERO, Scalar::ONE, Scalar::ZERO),
            Err(MathScalarRefusal::InvalidConfiguration)
        );
        let radius = Scalar::parse_decimal("0.05").unwrap();
        assert_eq!(
            deadband_scalar(radius.clone(), radius.clone()),
            Ok(Scalar::ZERO)
        );
        let outside = Scalar::parse_decimal("0.0500001").unwrap();
        assert_eq!(deadband_scalar(outside.clone(), radius), Ok(outside));
    }

    #[cfg(feature = "plot-catalog")]
    #[test]
    fn contracts_and_authored_signed_configuration_are_exact() {
        for contract in [
            math_clamp_contract(),
            math_scale_contract(),
            math_deadband_contract(),
        ] {
            assert_eq!(contract.inputs[0].value_kind.as_str(), SCALAR_INFO_ID);
            assert_eq!(contract.outputs[0].value_kind.as_str(), SCALAR_INFO_ID);
            assert_eq!(contract.inputs[0].temporal, PortTemporal::Value);
            assert_eq!(contract.outputs[0].temporal, PortTemporal::Value);
        }
        for contract in [
            math_clamp_semantic_contract(),
            math_scale_semantic_contract(),
            math_deadband_semantic_contract(),
        ] {
            assert!(conduit_core::pure_expression_facts(&contract).is_ok());
        }

        let mut startup = conduit_plot::StartupCatalog::new();
        let mut profile = conduit_plot::ProfileCatalog::new();
        install_math_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_plot::parse(
            "plot math {\n clamp: math/clamp(minimum = -7, maximum = 9)\n}\n",
            &profile,
        )
        .expect("signed scalar configuration checks");
        assert_eq!(
            checked.gears[0].configuration[0].value,
            ConfigurationValue::Scalar(Scalar::integer(-7))
        );
        assert!(conduit_plot::parse(
            "plot math {\n scale: math/scale(gain = 9223372036854775808)\n}\n",
            &profile,
        )
        .is_ok());
    }
}
