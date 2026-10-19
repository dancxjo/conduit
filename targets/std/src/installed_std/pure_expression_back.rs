//! Installed local execution for exact checked pure expressions.

use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::{ConfigurationValue, PlannedGear, MAXIMUM_STRUCTURED_CANONICAL_BYTES};

pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static SCALAR_TAPE_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) static FILTER_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION,
    budget,
    prepare,
};

pub(super) use conduit_semantic_catalog::PureExpressionBack;

pub(super) struct PureExpressionHost {
    evaluator: conduit_plot::PreparedPortableExpressionEvaluator,
    filter_output: Option<PreparedFilterOutput>,
}

enum PreparedFilterOutput {
    Flow(Vec<u8>),
    Value(conduit_core::PreparedOptionalInfoEncoder),
}

pub(super) fn prepare_hosts(
    fragment: &conduit_core::PlanFragment,
) -> Result<Vec<Option<PureExpressionHost>>, String> {
    fragment
        .placements
        .iter()
        .map(|placement| {
            if matches!(
                placement.implementation_id.as_str(),
                conduit_std_offers::PURE_EXPRESSION_STD_IMPLEMENTATION
                    | conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
                    | conduit_std_offers::PURE_FILTER_STD_IMPLEMENTATION
            ) {
                PureExpressionHost::from_placement(placement).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

impl PureExpressionHost {
    fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        let program = program_from_placement(placement)?;
        validate_placement(placement, &program)?;
        let filter_output = if placement.kind_contract_revision.as_str()
            == conduit_plot::PURE_FILTER_REVISION
        {
            Some(match placement.inputs[0].temporal {
                conduit_core::PortTemporal::Value => PreparedFilterOutput::Value(
                    conduit_core::PreparedOptionalInfoEncoder::new(program.input_type.clone())
                        .map_err(|error| format!("prepare optional filter output: {error:?}"))?,
                ),
                conduit_core::PortTemporal::Flow { .. } => PreparedFilterOutput::Flow(
                    Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
                ),
                conduit_core::PortTemporal::Current => {
                    return Err("when filter does not admit current-value temporal input".into())
                }
            })
        } else {
            None
        };
        Ok(Self {
            evaluator: if placement.implementation_id.as_str()
                == conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
            {
                conduit_plot::PreparedPortableExpressionEvaluator::new_scalar_tape(&program)
            } else if placement.execution_profile_id.as_str()
                == conduit_std_offers::PURE_EXACT_QUANTITY_STD_PROFILE
            {
                conduit_plot::PreparedPortableExpressionEvaluator::new_with_exact_quantities(
                    &program,
                )
            } else if placement.execution_profile_id.as_str()
                == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
            {
                conduit_plot::PreparedPortableExpressionEvaluator::new_with_mathematical_angles(
                    &program,
                )
            } else if placement.execution_profile_id.as_str()
                == conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
            {
                conduit_std_offers::prepare_near_contract_expression(&program)
                    .map_err(conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType)
            } else {
                conduit_plot::PreparedPortableExpressionEvaluator::new(&program)
            }
            .map_err(|error| format!("prepare pure expression evaluator: {error:?}"))?,
            filter_output,
        })
    }

    pub(super) fn execute(
        &mut self,
        input: &[u8],
    ) -> Result<&[u8], conduit_plot::PortableExpressionEvaluationRefusal> {
        self.evaluator.evaluate(input)
    }

    pub(super) fn execute_filter(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, conduit_plot::PortableExpressionEvaluationRefusal> {
        let predicate = self.evaluator.evaluate(input)?;
        let selected = conduit_core::InfoBool::decode(predicate)
            .map_err(|_| conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram)?
            .get();
        match self
            .filter_output
            .as_mut()
            .ok_or(conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram)?
        {
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
                .map_err(|_| conduit_plot::PortableExpressionEvaluationRefusal::InvalidInput),
        }
    }
}

pub(super) fn program_from_placement(
    placement: &PlannedGear,
) -> Result<conduit_plot::PortableExpressionProgram, String> {
    let [entry] = placement.configuration.as_slice() else {
        return Err("pure expression requires one exact planned configuration".into());
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err("pure expression planned configuration is malformed".into());
    };
    conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|error| format!("pure expression program refusal: {error:?}"))
}

fn validate_placement(
    placement: &PlannedGear,
    program: &conduit_plot::PortableExpressionProgram,
) -> Result<(), String> {
    if placement.execution_profile_id.as_str() == conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
    {
        crate::StdHost::validate_prepared_expression_annotations(program)?;
    }
    let temporal = placement
        .inputs
        .first()
        .map(|port| port.temporal)
        .ok_or("pure expression input is absent")?;
    let offer = if placement.implementation_id.as_str()
        == conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
    {
        conduit_std_offers::pure_scalar_tape_std_offer(program, temporal)?
    } else if placement.execution_profile_id.as_str()
        == conduit_std_offers::PURE_EXACT_QUANTITY_STD_PROFILE
    {
        conduit_std_offers::pure_exact_quantity_std_offer(program, temporal)?
    } else if placement.execution_profile_id.as_str()
        == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
    {
        conduit_std_offers::pure_mathematical_angle_std_offer(program, temporal)?
    } else if placement.execution_profile_id.as_str()
        == conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
    {
        conduit_std_offers::pure_near_contract_std_offer(program, temporal)?
    } else {
        let result =
            if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
                conduit_std_offers::pure_filter_std_offer(program, temporal)
            } else {
                conduit_std_offers::pure_expression_std_offer(program, temporal)
            };
        result.map_err(|error| format!("pure expression offer refusal: {error:?}"))?
    };
    let wire_kind = conduit_core::QuantityWireContract::property_kind();
    let predicate_kind = conduit_std_offers::pure_expression_value_contract_bounds_profile();
    if offer.realization_properties.iter().any(|expected| {
        !placement
            .realization_properties
            .iter()
            .any(|actual| actual == expected)
    }) || [&wire_kind, &predicate_kind].iter().any(|kind| {
        placement
            .realization_properties
            .iter()
            .any(|p| p.profile() == *kind)
            && !offer
                .realization_properties
                .iter()
                .any(|p| p.profile() == *kind)
    }) {
        return Err(
            "planned pure expression has missing or foreign prepared wire/resource facts".into(),
        );
    }
    if placement.semantic_contract.laws != offer.semantic_contract.laws
        || placement.kind_id != offer.kind_id
        || placement.kind_contract_revision != offer.kind_contract_revision
        || placement.execution_profile_id != offer.implementation.execution_profile_id
        || placement.implementation_id != offer.implementation.implementation_id
        || placement.artifact_id != offer.implementation.artifact_id
        || placement.inputs != offer.inputs
        || placement.outputs != offer.outputs
        || placement.host_calls != offer.host_calls
        || (placement.execution_profile_id.as_str()
            == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
            && placement.limits != offer.limits)
        || (placement.execution_profile_id.as_str()
            == conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
            && (placement.limits != offer.limits
                || placement.realization_properties != offer.realization_properties))
        || (placement.implementation_id.as_str()
            == conduit_std_offers::PURE_SCALAR_TAPE_STD_IMPLEMENTATION
            && placement.realization_properties != offer.realization_properties)
        || placement.inputs[0].temporal != placement.outputs[0].temporal
    {
        return Err("planned pure expression differs from installed realization".into());
    }
    Ok(())
}

fn budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let program = program_from_placement(placement)?;
    validate_placement(placement, &program)?;
    Ok(BackBudget {
        value_items: 8,
        value_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 8) as u32,
        host_requests: 1,
        sign_items: 32,
        maximum_value_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
    })
}

fn prepare(
    placement: &PlannedGear,
    _values: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(placement)?;
    if placement.kind_contract_revision.as_str() == conduit_plot::PURE_FILTER_REVISION {
        Ok(InstalledBack::PureFilter(
            conduit_semantic_catalog::StructuredSelectorBack::new(
                MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            ),
        ))
    } else {
        let maximum_input_bytes = if placement.execution_profile_id.as_str()
            == conduit_std_offers::PURE_MATHEMATICAL_ANGLE_STD_PROFILE
            || placement.execution_profile_id.as_str()
                == conduit_std_offers::PURE_NEAR_CONTRACT_STD_PROFILE
        {
            placement
                .host_calls
                .first()
                .ok_or("mathematical Angle Host Call missing")?
                .maximum_input_bytes
        } else {
            MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32
        };
        Ok(InstalledBack::PureExpression(PureExpressionBack::new(
            maximum_input_bytes,
        )))
    }
}

pub(super) fn refusal_detail(refusal: &conduit_plot::PortableExpressionEvaluationRefusal) -> u16 {
    match refusal {
        conduit_plot::PortableExpressionEvaluationRefusal::InputValueConstraint(_)
        | conduit_plot::PortableExpressionEvaluationRefusal::OutputValueConstraint(_) => {
            conduit_plot::value_contract_failure_detail(refusal)
        }
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidInput => 1,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidProgram => 2,
        conduit_plot::PortableExpressionEvaluationRefusal::InvalidLiteral => 3,
        conduit_plot::PortableExpressionEvaluationRefusal::Arithmetic => 4,
        conduit_plot::PortableExpressionEvaluationRefusal::QuantityProduct(_)
        | conduit_plot::PortableExpressionEvaluationRefusal::QuantityProjection(_)
        | conduit_plot::PortableExpressionEvaluationRefusal::QuantityEncoding(_) => {
            conduit_plot::quantity_arithmetic_failure_detail(refusal)
        }
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedSemanticCall(_) => 5,
        conduit_plot::PortableExpressionEvaluationRefusal::UnsupportedType(_) => 6,
        conduit_plot::PortableExpressionEvaluationRefusal::UnprovedMathematicalComparison => 7,
        conduit_plot::PortableExpressionEvaluationRefusal::MathematicalArithmetic(reason) => {
            conduit_plot::mathematical_arithmetic_failure_detail(reason)
        }
        conduit_plot::PortableExpressionEvaluationRefusal::SquareRoot(reason) => {
            conduit_plot::square_root_failure_detail(reason)
        }
        conduit_plot::PortableExpressionEvaluationRefusal::InverseAngle(reason) => {
            conduit_plot::inverse_angle_failure_detail(reason)
        }
        conduit_plot::PortableExpressionEvaluationRefusal::AngleReference(reason) => match reason {
            conduit_core::AngleReferenceRefusal::NotLinearAngle => 16,
            conduit_core::AngleReferenceRefusal::SourceEvidence(_) => 17,
            conduit_core::AngleReferenceRefusal::Coordinate(_) => 18,
            conduit_core::AngleReferenceRefusal::Denotation(_) => 19,
        },
        // 8/9 are existing filter failures above. Keep the new Angle domain
        // causes separate rather than flattening an undefined pole to arithmetic.
        conduit_plot::PortableExpressionEvaluationRefusal::AngleTrigonometry(reason) => {
            match reason {
                conduit_core::AngleTrigonometryRefusal::NotAngle => 10,
                conduit_core::AngleTrigonometryRefusal::UnsupportedRole => 11,
                conduit_core::AngleTrigonometryRefusal::SourceEvidence(_) => 12,
                conduit_core::AngleTrigonometryRefusal::Coordinate(_) => 13,
                conduit_core::AngleTrigonometryRefusal::Denotation(
                    conduit_core::MathematicalValueRefusal::TrigonometricPole,
                ) => 14,
                conduit_core::AngleTrigonometryRefusal::Denotation(_) => 15,
            }
        }
    }
}

// Reuse the computational Back already used by admitted child kernels. Its
// exact local Offer is checked before storage is prepared; no Host Call is
// silently removed from a hosted realization.
pub(super) static LOCAL_FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::PURE_EXPRESSION_LOCAL_IMPLEMENTATION,
    budget: local_budget,
    prepare: prepare_local,
};
pub(super) fn local_budget(placement: &PlannedGear) -> Result<BackBudget, String> {
    let factory = crate::flow_activation::LocalExpressionFactory;
    let budget = conduit_composite::KernelOperationFactory::budget(&factory, placement)?;
    Ok(BackBudget {
        value_items: budget.value_items,
        value_bytes: budget.value_bytes,
        maximum_value_bytes: budget.maximum_value_bytes,
        host_requests: budget.host_requests as usize,
        sign_items: budget.sign_items,
    })
}
fn prepare_local(
    placement: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    local_budget(placement)?;
    Ok(InstalledBack::LocalExpression(
        crate::flow_activation::PreparedLocalExpressionBack::from_placement(placement)?,
    ))
}

#[cfg(test)]
#[path = "pure_expression_back/scalar_tape_tests.rs"]
mod scalar_tape_tests;

#[cfg(test)]
#[path = "pure_expression_back/scalar_tape_parity_tests.rs"]
mod scalar_tape_parity_tests;

#[cfg(test)]
#[path = "pure_expression_back/exact_quantity_tests.rs"]
mod exact_quantity_tests;

#[cfg(test)]
#[path = "pure_expression_back/mathematical_angle_tests.rs"]
mod mathematical_angle_tests;
