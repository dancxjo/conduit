//! Dynamic browser realization of exact checked pure expressions.

use super::factory::{BrowserHostResult, BrowserInstallation};
use super::BrowserBack;
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallContractId, HostCallRequirement, ImplementationId, PlannedGear,
    PortTemporal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_kernel::{Failure, FailureCode, HostedValueStore};

pub(crate) const IMPLEMENTATION: &str = "browser/kernel-pure-expression@1";
pub(crate) const HOST_CALL: &str = "conduit.host/browser-pure-expression@1";
pub(crate) const FILTER_IMPLEMENTATION: &str = "browser/kernel-pure-filter@1";
pub(crate) const FILTER_HOST_CALL: &str = "conduit.host/browser-pure-filter@1";

pub(super) static INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: Some(unreachable_perform),
};
pub(super) static FILTER_INSTALLATION: BrowserInstallation = BrowserInstallation {
    implementation_id: FILTER_IMPLEMENTATION,
    offer: unreachable_offer,
    prepare,
    perform: Some(unreachable_perform),
};

pub(crate) struct PreparedExpression {
    evaluator: conduit_plot::PreparedPortableExpressionEvaluator,
    filter_output: Option<PreparedFilterOutput>,
}

enum PreparedFilterOutput {
    Flow(Vec<u8>),
    Value(conduit_core::PreparedOptionalInfoEncoder),
}

impl PreparedExpression {
    pub(crate) fn for_placement(placement: &PlannedGear) -> Result<Option<Self>, String> {
        if !matches!(
            placement.implementation_id.as_str(),
            IMPLEMENTATION | FILTER_IMPLEMENTATION
        ) {
            return Ok(None);
        }
        let program = program_from_placement(placement)?;
        validate(placement, &program)?;
        let filter_output = if placement.kind_contract_revision.as_str()
            == conduit_plot::PURE_FILTER_REVISION
        {
            Some(match placement.inputs[0].temporal {
                PortTemporal::Value => PreparedFilterOutput::Value(
                    conduit_core::PreparedOptionalInfoEncoder::new(program.input_type.clone())
                        .map_err(|error| format!("prepare optional filter output: {error:?}"))?,
                ),
                PortTemporal::Flow { .. } => PreparedFilterOutput::Flow(Vec::with_capacity(
                    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
                )),
                PortTemporal::Current => {
                    return Err("when filter does not admit current-value temporal input".into())
                }
            })
        } else {
            None
        };
        Ok(Some(Self {
            evaluator: conduit_plot::PreparedPortableExpressionEvaluator::new(&program)
                .map_err(|error| format!("prepare pure expression evaluator: {error:?}"))?,
            filter_output,
        }))
    }

    pub(crate) fn execute(&mut self, input: &[u8]) -> Result<&[u8], Failure> {
        self.evaluator.evaluate(input).map_err(failure)
    }

    pub(crate) fn execute_filter(&mut self, input: &[u8]) -> Result<Option<&[u8]>, Failure> {
        let selected =
            conduit_core::InfoBool::decode(self.evaluator.evaluate(input).map_err(failure)?)
                .map(conduit_core::InfoBool::get)
                .map_err(|_| Failure {
                    code: FailureCode::InvalidInput,
                    detail: 7,
                })?;
        match self.filter_output.as_mut().ok_or(Failure {
            code: FailureCode::InvalidInput,
            detail: 8,
        })? {
            PreparedFilterOutput::Flow(output) => {
                if !selected {
                    return Ok(None);
                }
                output.clear();
                output.extend_from_slice(input);
                Ok(Some(output))
            }
            PreparedFilterOutput::Value(output) => output
                .encode(selected.then_some(input))
                .map(Some)
                .map_err(|_| Failure {
                    code: FailureCode::InvalidInput,
                    detail: 9,
                }),
        }
    }
}

pub(crate) fn offer(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    let contract = conduit_semantic_catalog::pure_expression_contract(program, temporal)
        .map_err(|error| format!("pure expression contract: {error:?}"))?;
    let target = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("browser/{}", target.as_str())),
            execution_profile_id: ExecutionProfileId::from(
                "browser/pure-expression-kernel-hosted@1",
            ),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-plot/pure-expression@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(HOST_CALL),
                target_kind: Some(target),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

pub(crate) fn filter_offer(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    let contract = conduit_semantic_catalog::pure_filter_contract(program, temporal)
        .map_err(|error| format!("pure filter contract: {error:?}"))?;
    let target = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("browser/{}", target.as_str())),
            execution_profile_id: ExecutionProfileId::from("browser/pure-filter-kernel-hosted@1"),
            implementation_id: ImplementationId::from(FILTER_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-plot/pure-filter@1"),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(FILTER_HOST_CALL),
                target_kind: Some(target),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}

pub(crate) fn offer_for_placement(
    placement: &PlannedGear,
) -> Result<Option<CapabilityOffer>, String> {
    if !matches!(
        placement.implementation_id.as_str(),
        IMPLEMENTATION | FILTER_IMPLEMENTATION
    ) {
        return Ok(None);
    }
    let program = program_from_placement(placement)?;
    validate(placement, &program)?;
    let exact = if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
        filter_offer(&program, placement.inputs[0].temporal)?
    } else {
        offer(&program, placement.inputs[0].temporal)?
    };
    if placement.capability_id != exact.capability_id {
        return Err("planned pure expression capability differs from its exact contract".into());
    }
    Ok(Some(exact))
}

pub(crate) fn program_from_configuration(
    configuration: &[conduit_core::ConfigurationEntry],
) -> Result<conduit_plot::PortableExpressionProgram, String> {
    let [entry] = configuration else {
        return Err("pure expression requires one exact configuration".into());
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("pure expression configuration is malformed".into());
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|error| format!("pure expression configuration: {error:?}"))
}

fn program_from_placement(
    placement: &PlannedGear,
) -> Result<conduit_plot::PortableExpressionProgram, String> {
    program_from_configuration(&placement.configuration)
}

fn validate(
    placement: &PlannedGear,
    program: &conduit_plot::PortableExpressionProgram,
) -> Result<(), String> {
    let temporal = placement
        .inputs
        .first()
        .map(|port| port.temporal)
        .ok_or("pure expression input is absent")?;
    let exact = if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
        filter_offer(program, temporal)?
    } else {
        offer(program, temporal)?
    };
    if placement.kind_id != exact.kind_id
        || placement.kind_contract_revision != exact.kind_contract_revision
        || placement.execution_profile_id != exact.implementation.execution_profile_id
        || placement.implementation_id != exact.implementation.implementation_id
        || placement.artifact_id != exact.implementation.artifact_id
        || placement.inputs != exact.inputs
        || placement.outputs != exact.outputs
        || placement.host_calls != exact.host_calls
    {
        return Err("planned pure expression differs from browser realization".into());
    }
    Ok(())
}

fn prepare(placement: &PlannedGear, _: &mut HostedValueStore) -> Result<BrowserBack, String> {
    let program = program_from_placement(placement)?;
    validate(placement, &program)?;
    if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
        Ok(BrowserBack::installed_step(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ))
    } else {
        Ok(BrowserBack::installed_step(
            conduit_semantic_catalog::PureExpressionBack::new(
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ))
    }
}

fn failure(refusal: conduit_plot::PortableExpressionEvaluationRefusal) -> Failure {
    let detail = match refusal {
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidInput => 1,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram => 2,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidLiteral => 3,
        conduit_plot::PortableExpressionEvaluationRefusal::Arithmetic => 4,
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedSemanticCall(_) => 5,
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_) => 6,
    };
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

fn unreachable_offer() -> CapabilityOffer {
    panic!("dynamic pure-expression offers must be derived from checked source")
}

fn unreachable_perform(_: &PlannedGear, _: &[u8]) -> Result<BrowserHostResult, String> {
    Err("pure expression execution requires prepared exact program state".into())
}
