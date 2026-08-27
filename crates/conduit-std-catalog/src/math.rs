//! Exact bounded scalar-control contracts and no-std semantic functions.

use super::{
    StandardConfigurationField, StandardConfigurationRule, StandardKindContract, TerminalBehavior,
};
#[cfg(feature = "form-catalog")]
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationValue, ExecutionProfileId, HostOperationContractId, HostOperationRequirement,
    ImplementationId, KindContractRevision, PortDescriptor, PortDirection, PortTemporal, Scalar,
    ScalarArithmeticError, SCALAR_ENCODED_LEN, SCALAR_INFO_ID,
};

pub const MATH_CLAMP_KIND: &str = "math/clamp";
pub const MATH_SCALE_KIND: &str = "math/scale";
pub const MATH_DEADBAND_KIND: &str = "math/deadband";

pub const MATH_CLAMP_CONTRACT_REVISION: &str = "conduit.std/math-clamp-scalar@1";
pub const MATH_CLAMP_EXECUTION_PROFILE: &str = "conduit.std/math-clamp-scalar-kernel@1";
pub const MATH_CLAMP_IMPLEMENTATION: &str = "std/kernel-math-clamp-scalar@1";
pub const MATH_CLAMP_ARTIFACT: &str = "conduit-std-host/math-clamp-scalar@1";
pub const MATH_CLAMP_CAPABILITY: &str = "math-clamp-scalar-v1";
pub const MATH_CLAMP_HOST_OPERATION: &str = "conduit.host/math-clamp-scalar@1";

pub const MATH_SCALE_CONTRACT_REVISION: &str = "conduit.std/math-scale-scalar@1";
pub const MATH_SCALE_EXECUTION_PROFILE: &str = "conduit.std/math-scale-scalar-kernel@1";
pub const MATH_SCALE_IMPLEMENTATION: &str = "std/kernel-math-scale-scalar@1";
pub const MATH_SCALE_ARTIFACT: &str = "conduit-std-host/math-scale-scalar@1";
pub const MATH_SCALE_CAPABILITY: &str = "math-scale-scalar-v1";
pub const MATH_SCALE_HOST_OPERATION: &str = "conduit.host/math-scale-scalar@1";

pub const MATH_DEADBAND_CONTRACT_REVISION: &str = "conduit.std/math-deadband-scalar@1";
pub const MATH_DEADBAND_EXECUTION_PROFILE: &str = "conduit.std/math-deadband-scalar-kernel@1";
pub const MATH_DEADBAND_IMPLEMENTATION: &str = "std/kernel-math-deadband-scalar@1";
pub const MATH_DEADBAND_ARTIFACT: &str = "conduit-std-host/math-deadband-scalar@1";
pub const MATH_DEADBAND_CAPABILITY: &str = "math-deadband-scalar-v1";
pub const MATH_DEADBAND_HOST_OPERATION: &str = "conduit.host/math-deadband-scalar@1";

pub const SCALAR_INPUT_PORT: &str = "in";
pub const SCALAR_OUTPUT_PORT: &str = "out";
pub const CLAMP_MINIMUM_KEY: &str = "minimum";
pub const CLAMP_MAXIMUM_KEY: &str = "maximum";
pub const SCALE_GAIN_KEY: &str = "gain";
pub const DEADBAND_RADIUS_KEY: &str = "radius";

pub const DEFAULT_CLAMP_MINIMUM: i64 = -Scalar::SCALE;
pub const DEFAULT_CLAMP_MAXIMUM: i64 = Scalar::SCALE;
pub const DEFAULT_SCALE_GAIN: i64 = Scalar::SCALE;
pub const DEFAULT_DEADBAND_RADIUS: i64 = 50_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathScalarError {
    InvalidConfiguration,
    Overflow,
}

impl From<ScalarArithmeticError> for MathScalarError {
    fn from(_: ScalarArithmeticError) -> Self {
        Self::Overflow
    }
}

pub fn clamp_scalar(
    input: Scalar,
    minimum: Scalar,
    maximum: Scalar,
) -> Result<Scalar, MathScalarError> {
    if minimum > maximum {
        return Err(MathScalarError::InvalidConfiguration);
    }
    Ok(input.max(minimum).min(maximum))
}

pub fn scale_scalar(input: Scalar, gain: Scalar) -> Result<Scalar, MathScalarError> {
    input.checked_mul(gain).map_err(Into::into)
}

pub fn deadband_scalar(input: Scalar, radius: Scalar) -> Result<Scalar, MathScalarError> {
    let radius = radius.raw_microunits();
    if radius < 0 {
        return Err(MathScalarError::InvalidConfiguration);
    }
    let raw = input.raw_microunits();
    Ok(if raw >= -radius && raw <= radius {
        Scalar::ZERO
    } else {
        input
    })
}

pub fn math_clamp_contract() -> StandardKindContract {
    contract(
        MATH_CLAMP_KIND,
        "Clamp scalar",
        "Limit one scalar to an inclusive configured minimum and maximum.",
        vec![
            scalar_field(CLAMP_MINIMUM_KEY, DEFAULT_CLAMP_MINIMUM, i64::MIN, i64::MAX),
            scalar_field(CLAMP_MAXIMUM_KEY, DEFAULT_CLAMP_MAXIMUM, i64::MIN, i64::MAX),
        ],
        "clamp: math/clamp(minimum = -1000000, maximum = 1000000)",
    )
}

pub fn math_scale_contract() -> StandardKindContract {
    contract(
        MATH_SCALE_KIND,
        "Scale scalar",
        "Multiply one scalar by a configured fixed-point gain with checked overflow.",
        vec![scalar_field(
            SCALE_GAIN_KEY,
            DEFAULT_SCALE_GAIN,
            i64::MIN,
            i64::MAX,
        )],
        "scale: math/scale(gain = 1000000)",
    )
}

pub fn math_deadband_contract() -> StandardKindContract {
    contract(
        MATH_DEADBAND_KIND,
        "Scalar deadband",
        "Emit zero inside an inclusive symmetric radius and preserve values outside it.",
        vec![scalar_field(
            DEADBAND_RADIUS_KEY,
            DEFAULT_DEADBAND_RADIUS,
            0,
            i64::MAX,
        )],
        "deadband: math/deadband(radius = 50000)",
    )
}

pub fn math_clamp_offer() -> CapabilityOffer {
    offer(
        math_clamp_contract(),
        MATH_CLAMP_CAPABILITY,
        MATH_CLAMP_CONTRACT_REVISION,
        MATH_CLAMP_EXECUTION_PROFILE,
        MATH_CLAMP_IMPLEMENTATION,
        MATH_CLAMP_ARTIFACT,
        MATH_CLAMP_HOST_OPERATION,
    )
}

pub fn math_scale_offer() -> CapabilityOffer {
    offer(
        math_scale_contract(),
        MATH_SCALE_CAPABILITY,
        MATH_SCALE_CONTRACT_REVISION,
        MATH_SCALE_EXECUTION_PROFILE,
        MATH_SCALE_IMPLEMENTATION,
        MATH_SCALE_ARTIFACT,
        MATH_SCALE_HOST_OPERATION,
    )
}

pub fn math_deadband_offer() -> CapabilityOffer {
    offer(
        math_deadband_contract(),
        MATH_DEADBAND_CAPABILITY,
        MATH_DEADBAND_CONTRACT_REVISION,
        MATH_DEADBAND_EXECUTION_PROFILE,
        MATH_DEADBAND_IMPLEMENTATION,
        MATH_DEADBAND_ARTIFACT,
        MATH_DEADBAND_HOST_OPERATION,
    )
}

fn contract(
    kind: &str,
    plain_name: &str,
    summary: &str,
    configuration: alloc::vec::Vec<StandardConfigurationField>,
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
            max_queue_bytes: SCALAR_ENCODED_LEN as u32,
        },
        terminal_behavior:
            TerminalBehavior::EmitsOneDecisionOrCompletesWhenDecisionBecomesImpossible,
        hosted_implementation_required: true,
        browser_manifestation_honest: false,
        pico_manifestation_honest: false,
        example: example.to_string(),
    }
}

fn scalar_field(key: &str, default: i64, minimum: i64, maximum: i64) -> StandardConfigurationField {
    StandardConfigurationField {
        key: key.to_string(),
        default_value: ConfigurationValue::I64(default),
        rule: StandardConfigurationRule::I64Range { minimum, maximum },
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(SCALAR_INFO_ID),
        direction,
        temporal: PortTemporal::Value,
    }
}

#[allow(clippy::too_many_arguments)]
fn offer(
    contract: StandardKindContract,
    capability: &str,
    revision: &str,
    profile: &str,
    implementation: &str,
    artifact: &str,
    operation: &str,
) -> CapabilityOffer {
    let target = contract.kind_id.clone();
    CapabilityOffer {
        startup_parameters: super::functional_face::startup_face(&contract.configuration),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(revision),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(profile),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(artifact),
        },
        inputs: contract.inputs,
        outputs: contract.outputs,
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(operation),
            target_kind: Some(target),
            maximum_in_flight: 1,
            maximum_input_bytes: SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: SCALAR_ENCODED_LEN as u32,
        }],
        resource_requirements: alloc::vec::Vec::new(),
        authority_requirements: alloc::vec::Vec::new(),
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_math_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, KindSignature};
    for (contract, revision) in [
        (math_clamp_contract(), MATH_CLAMP_CONTRACT_REVISION),
        (math_scale_contract(), MATH_SCALE_CONTRACT_REVISION),
        (math_deadband_contract(), MATH_DEADBAND_CONTRACT_REVISION),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().to_string(),
            startup_parameters: contract
                .configuration
                .iter()
                .map(|field| conduit_form::StartupParameterSignature {
                    name: field.key.clone(),
                    value_type: "Scalar".to_string(),
                    default: Some(match field.default_value {
                        ConfigurationValue::I64(value) => value.to_string(),
                        _ => unreachable!("math configuration is signed scalar microunits"),
                    }),
                })
                .collect(),
        })?;
        let configuration = contract
            .configuration
            .into_iter()
            .map(|field| ConfigurationField {
                key: field.key,
                default_value: field.default_value,
                validation: match field.rule {
                    StandardConfigurationRule::I64Range { minimum, maximum } => {
                        ConfigurationRule::I64Range { minimum, maximum }
                    }
                    _ => unreachable!("math configuration has signed finite bounds"),
                },
            })
            .collect();
        profile
            .insert(KindDefinition {
                kind_id: contract.kind_id,
                kind_contract_revision: KindContractRevision::from(revision),
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration,
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_boundaries_and_overflow_are_exact() {
        let one = Scalar::ONE;
        let negative_one = Scalar::from_raw_microunits(-Scalar::SCALE);
        assert_eq!(
            clamp_scalar(Scalar::MIN, negative_one, one),
            Ok(negative_one)
        );
        assert_eq!(clamp_scalar(Scalar::MAX, negative_one, one), Ok(one));
        assert_eq!(
            clamp_scalar(one, one, negative_one),
            Err(MathScalarError::InvalidConfiguration)
        );
        assert_eq!(
            scale_scalar(
                Scalar::from_raw_microunits(500_000),
                Scalar::from_raw_microunits(2_000_000)
            ),
            Ok(one)
        );
        assert_eq!(
            scale_scalar(Scalar::MAX, Scalar::from_raw_microunits(2_000_000)),
            Err(MathScalarError::Overflow)
        );
    }

    #[test]
    fn deadband_is_inclusive_and_minimum_remains_representable() {
        let radius = Scalar::from_raw_microunits(50_000);
        assert_eq!(
            deadband_scalar(Scalar::from_raw_microunits(-50_000), radius),
            Ok(Scalar::ZERO)
        );
        assert_eq!(
            deadband_scalar(Scalar::from_raw_microunits(50_000), radius),
            Ok(Scalar::ZERO)
        );
        assert_eq!(
            deadband_scalar(Scalar::from_raw_microunits(50_001), radius),
            Ok(Scalar::from_raw_microunits(50_001))
        );
        assert_eq!(deadband_scalar(Scalar::MIN, Scalar::MAX), Ok(Scalar::MIN));
    }

    #[cfg(feature = "form-catalog")]
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

        let mut startup = conduit_form::StartupCatalog::new();
        let mut profile = conduit_form::ProfileCatalog::new();
        install_math_catalogs(&mut startup, &mut profile).unwrap();
        let checked = conduit_form::parse(
            "form math {\n clamp: math/clamp(minimum = -7, maximum = 9)\n}\n",
            &profile,
        )
        .expect("signed scalar configuration checks");
        assert_eq!(
            checked.gears[0].configuration[0].value,
            ConfigurationValue::I64(-7)
        );
        assert!(conduit_form::parse(
            "form math {\n scale: math/scale(gain = 9223372036854775808)\n}\n",
            &profile,
        )
        .is_err());
    }
}
