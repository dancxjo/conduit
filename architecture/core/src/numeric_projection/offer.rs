//! Offered algorithms are declarations; only recomputed witnesses authorize a
//! selected numerical realization. Both enter the sealed Plan as typed records.
use super::{configuration::*, *};
use crate::numeric_realization::{
    certify_computation_for_algorithm, verify_encoded_computation_claim, NumericAlgorithm,
    NumericEngineRefusal, VerifiedNumericWitness,
};
use crate::{
    NumericComputation, StructuredConfigurationValue, StructuredInfoType as Type,
    StructuredInfoValue as Value,
};
use alloc::vec;

pub const NUMERIC_TENSOR_MATERIALIZATION_RESERVATION_BYTES: u32 = 20_480;

pub const NUMERIC_REALIZATION_OFFER_ID: &str = "numeric/realization-offer@1";
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumericRealizationOffer {
    pub algorithm: NumericAlgorithm,
    pub budget: NumericExecutionBudget,
}
pub fn numeric_realization_offer_info_type() -> Type {
    record(
        NUMERIC_REALIZATION_OFFER_ID,
        vec![
            ("algorithm", leaf(crate::TEXT_INFO_ID)),
            ("workUnit", leaf(crate::TEXT_INFO_ID)),
            ("budget", budget_type()),
        ],
    )
}
pub fn numeric_realization_offer_profile() -> crate::KindId {
    numeric_realization_offer_info_type()
        .profile()
        .expect("bounded offer schema")
        .value_kind()
        .clone()
}
pub fn numeric_realization_property_info_type() -> Type {
    record(
        NUMERIC_REALIZATION_PROPERTY_ID,
        vec![
            ("claim", leaf(crate::BYTES_INFO_ID)),
            ("graphIdentity", leaf(crate::BYTES_INFO_ID)),
            ("requirementIdentity", leaf(crate::BYTES_INFO_ID)),
        ],
    )
}
pub fn numeric_realization_property_profile() -> crate::KindId {
    numeric_realization_property_info_type()
        .profile()
        .expect("bounded witness schema")
        .value_kind()
        .clone()
}
fn seal(value: Value) -> Result<StructuredConfigurationValue, NumericRefusal> {
    let profile = value
        .value_type()
        .profile()
        .map_err(invalid)?
        .value_kind()
        .clone();
    let encoded = value.canonical_bytes().map_err(invalid)?;
    if encoded.len() > crate::MAXIMUM_REALIZATION_PROPERTY_BYTES {
        return Err(NumericRefusal::StorageBudget);
    }
    StructuredConfigurationValue::new(profile, encoded).ok_or(NumericRefusal::MalformedEncoding)
}
impl NumericRealizationOffer {
    pub fn configuration(&self) -> Result<StructuredConfigurationValue, NumericRefusal> {
        self.budget.validate()?;
        let b = self.budget;
        seal(value_record(
            numeric_realization_offer_info_type(),
            vec![
                (
                    "algorithm",
                    value_leaf(
                        crate::TEXT_INFO_ID,
                        self.algorithm.identity().as_bytes().to_vec(),
                    )?,
                ),
                (
                    "workUnit",
                    value_leaf(
                        crate::TEXT_INFO_ID,
                        NUMERIC_WORK_UNIT_ID.as_bytes().to_vec(),
                    )?,
                ),
                (
                    "budget",
                    value_record(
                        budget_type(),
                        vec![
                            (
                                "storageBytes",
                                value_leaf("value/u32", b.storage_bytes.to_le_bytes().to_vec())?,
                            ),
                            (
                                "workSteps",
                                value_leaf("value/u64", b.work_steps.to_le_bytes().to_vec())?,
                            ),
                            (
                                "limbs",
                                value_leaf("value/u16", b.limbs.to_le_bytes().to_vec())?,
                            ),
                            (
                                "symbolicNodes",
                                value_leaf("value/u16", b.symbolic_nodes.to_le_bytes().to_vec())?,
                            ),
                        ],
                    )?,
                ),
            ],
        )?)
    }
    pub fn from_configuration(
        config: &StructuredConfigurationValue,
    ) -> Result<Self, NumericRefusal> {
        if config.profile() != &numeric_realization_offer_profile() {
            return Err(NumericRefusal::MalformedEncoding);
        }
        let value = Value::from_canonical_bytes(config.canonical_value()).map_err(invalid)?;
        if value.value_type() != &numeric_realization_offer_info_type()
            || bytes(field(&value, "workUnit")?)? != NUMERIC_WORK_UNIT_ID.as_bytes()
        {
            return Err(NumericRefusal::InvalidProfile);
        }
        let identity = bytes(field(&value, "algorithm")?)?;
        let algorithm = [
            NumericAlgorithm::OrderedCheckedI64V1,
            NumericAlgorithm::OrderedRationalV1,
            NumericAlgorithm::SymbolicRetentionV1,
            NumericAlgorithm::OrderedSoftwareBinary64V1,
            NumericAlgorithm::CertifiedRationalIntervalV1,
            NumericAlgorithm::ExactComparisonV1,
            NumericAlgorithm::OrderedIeeeSoftwareBinary64V1,
        ]
        .into_iter()
        .find(|a| a.identity().as_bytes() == identity)
        .ok_or(NumericRefusal::InvalidProfile)?;
        let b = field(&value, "budget")?;
        let budget = NumericExecutionBudget {
            storage_bytes: u32::from_le_bytes(integer(field(b, "storageBytes")?)?),
            work_steps: u64::from_le_bytes(integer(field(b, "workSteps")?)?),
            limbs: u16::from_le_bytes(integer(field(b, "limbs")?)?),
            symbolic_nodes: u16::from_le_bytes(integer(field(b, "symbolicNodes")?)?),
        };
        budget.validate()?;
        Ok(Self { algorithm, budget })
    }
    pub fn certify(
        &self,
        graph: &NumericComputation,
        requirement: &NumericRequirement,
    ) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
        self.budget.validate()?;
        let witness = certify_computation_for_algorithm(graph, requirement, self.algorithm)?;
        if witness.claim().storage_bytes > self.budget.storage_bytes {
            return Err(NumericRefusal::StorageBudget.into());
        }
        if witness.claim().work_steps > self.budget.work_steps {
            return Err(NumericRefusal::WorkBudget.into());
        }
        if witness.required_limb_capacity() > self.budget.limbs {
            return Err(NumericRefusal::IntegerBudget.into());
        }
        if witness.required_symbolic_nodes() > self.budget.symbolic_nodes {
            return Err(NumericRefusal::NodeBudget.into());
        }
        // A candidate that cannot carry its evidence cannot enter the Plan.
        numeric_witness_configuration(&witness)?;
        Ok(witness)
    }
}
pub fn numeric_witness_configuration(
    witness: &VerifiedNumericWitness,
) -> Result<StructuredConfigurationValue, NumericEngineRefusal> {
    Ok(seal(value_record(
        numeric_realization_property_info_type(),
        vec![
            (
                "claim",
                value_leaf(crate::BYTES_INFO_ID, witness.claim().canonical_bytes()?)?,
            ),
            (
                "graphIdentity",
                value_leaf(crate::BYTES_INFO_ID, witness.graph_identity().to_vec())?,
            ),
            (
                "requirementIdentity",
                value_leaf(
                    crate::BYTES_INFO_ID,
                    witness.requirement_identity().to_vec(),
                )?,
            ),
        ],
    )?)?)
}
pub fn verify_numeric_witness_configuration(
    config: &StructuredConfigurationValue,
    graph: &NumericComputation,
    requirement: &NumericRequirement,
) -> Result<VerifiedNumericWitness, NumericEngineRefusal> {
    if config.profile() != &numeric_realization_property_profile() {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    let value = Value::from_canonical_bytes(config.canonical_value()).map_err(invalid)?;
    if value.value_type() != &numeric_realization_property_info_type() {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    let witness =
        verify_encoded_computation_claim(bytes(field(&value, "claim")?)?, graph, requirement)?;
    if bytes(field(&value, "graphIdentity")?)? != witness.graph_identity()
        || bytes(field(&value, "requirementIdentity")?)? != witness.requirement_identity()
    {
        return Err(NumericEngineRefusal::ClaimMismatch);
    }
    Ok(witness)
}

pub fn numeric_tensor_witness_info_type() -> Type {
    record(
        "numeric/tensor-witness@1",
        vec![
            ("claims", leaf(crate::BYTES_INFO_ID)),
            ("graphIdentities", leaf(crate::BYTES_INFO_ID)),
            ("requirementIdentity", leaf(crate::BYTES_INFO_ID)),
        ],
    )
}
impl NumericRealizationOffer {
    /// Coordinate proofs share one consumer's total storage and work budget.
    pub fn certify_tensor(
        &self,
        graphs: &[NumericComputation],
        requirement: &NumericRequirement,
    ) -> Result<alloc::vec::Vec<VerifiedNumericWitness>, NumericEngineRefusal> {
        if graphs.is_empty() || graphs.len() > 8 {
            return Err(NumericEngineRefusal::InvalidComputation);
        }
        let mut witnesses = alloc::vec::Vec::with_capacity(graphs.len());
        let mut work = 0u64;
        let mut storage = 0u32;
        for graph in graphs {
            let witness = self.certify(graph, requirement)?;
            work = work
                .checked_add(witness.claim().work_steps)
                .ok_or(NumericRefusal::WorkBudget)?;
            storage = storage
                .checked_add(witness.claim().storage_bytes)
                .ok_or(NumericRefusal::StorageBudget)?;
            if work > self.budget.work_steps.min(requirement.budget.work_steps) {
                return Err(NumericRefusal::WorkBudget.into());
            }
            if storage
                > self
                    .budget
                    .storage_bytes
                    .min(requirement.budget.storage_bytes)
            {
                return Err(NumericRefusal::StorageBudget.into());
            }
            witnesses.push(witness);
        }
        numeric_tensor_witness_configuration(&witnesses)?;
        Ok(witnesses)
    }
}
pub fn numeric_tensor_witness_profile() -> crate::KindId {
    numeric_tensor_witness_info_type()
        .profile()
        .expect("bounded tensor witness schema")
        .value_kind()
        .clone()
}
pub fn numeric_tensor_witness_configuration(
    witnesses: &[VerifiedNumericWitness],
) -> Result<StructuredConfigurationValue, NumericEngineRefusal> {
    if witnesses.is_empty() || witnesses.len() > 8 {
        return Err(NumericEngineRefusal::InvalidComputation);
    }
    let requirement = witnesses[0].requirement_identity();
    let mut claims = alloc::vec![witnesses.len() as u8];
    let mut identities = alloc::vec::Vec::with_capacity(32 * witnesses.len());
    for witness in witnesses {
        if witness.requirement_identity() != requirement {
            return Err(NumericEngineRefusal::ClaimMismatch);
        }
        let claim = witness.claim().canonical_bytes()?;
        let size = u32::try_from(claim.len()).map_err(|_| NumericRefusal::StorageBudget)?;
        claims.extend_from_slice(&size.to_le_bytes());
        claims.extend_from_slice(&claim);
        if claims.len() > crate::MAXIMUM_REALIZATION_PROPERTY_BYTES {
            return Err(NumericRefusal::StorageBudget.into());
        }
        identities.extend_from_slice(witness.graph_identity());
    }
    Ok(seal(value_record(
        numeric_tensor_witness_info_type(),
        vec![
            ("claims", value_leaf(crate::BYTES_INFO_ID, claims)?),
            (
                "graphIdentities",
                value_leaf(crate::BYTES_INFO_ID, identities)?,
            ),
            (
                "requirementIdentity",
                value_leaf(crate::BYTES_INFO_ID, requirement.to_vec())?,
            ),
        ],
    )?)?)
}
pub fn verify_numeric_tensor_witness_configuration(
    config: &StructuredConfigurationValue,
    graphs: &[NumericComputation],
    requirement: &NumericRequirement,
) -> Result<alloc::vec::Vec<VerifiedNumericWitness>, NumericEngineRefusal> {
    if graphs.is_empty()
        || graphs.len() > 8
        || config.profile() != &numeric_tensor_witness_profile()
        || config.canonical_value().len() > crate::MAXIMUM_REALIZATION_PROPERTY_BYTES
    {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    let value = Value::from_canonical_bytes(config.canonical_value()).map_err(invalid)?;
    if value.value_type() != &numeric_tensor_witness_info_type() {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    let claims = bytes(field(&value, "claims")?)?;
    let identities = bytes(field(&value, "graphIdentities")?)?;
    if claims.first().copied() != Some(graphs.len() as u8)
        || identities.len() != 32 * graphs.len()
        || bytes(field(&value, "requirementIdentity")?)? != requirement.identity()?
    {
        return Err(NumericEngineRefusal::ClaimMismatch);
    }
    let mut remaining = &claims[1..];
    let mut verified = alloc::vec::Vec::with_capacity(graphs.len());
    for (index, graph) in graphs.iter().enumerate() {
        let length_bytes: [u8; 4] = remaining
            .get(..4)
            .ok_or(NumericRefusal::MalformedEncoding)?
            .try_into()
            .map_err(|_| NumericRefusal::MalformedEncoding)?;
        let length = u32::from_le_bytes(length_bytes) as usize;
        remaining = &remaining[4..];
        let claim = remaining
            .get(..length)
            .ok_or(NumericRefusal::MalformedEncoding)?;
        let witness = verify_encoded_computation_claim(claim, graph, requirement)?;
        if identities[index * 32..(index + 1) * 32] != *witness.graph_identity() {
            return Err(NumericEngineRefusal::ClaimMismatch);
        }
        verified.push(witness);
        remaining = &remaining[length..];
    }
    if !remaining.is_empty() {
        return Err(NumericRefusal::MalformedEncoding.into());
    }
    let work = verified
        .iter()
        .try_fold(0u64, |sum, witness| {
            sum.checked_add(witness.claim().work_steps)
        })
        .ok_or(NumericRefusal::WorkBudget)?;
    let storage = verified
        .iter()
        .try_fold(0u32, |sum, witness| {
            sum.checked_add(witness.claim().storage_bytes)
        })
        .ok_or(NumericRefusal::StorageBudget)?;
    if work > requirement.budget.work_steps {
        return Err(NumericRefusal::WorkBudget.into());
    }
    if storage > requirement.budget.storage_bytes {
        return Err(NumericRefusal::StorageBudget.into());
    }
    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::numeric_realization::Binary64Rounding;
    use crate::{MathValue, NumericComputationNode};
    use alloc::{string::ToString, vec::Vec};
    fn fixture() -> (
        NumericComputation,
        NumericRequirement,
        NumericRealizationOffer,
    ) {
        let graph = NumericComputation::new(
            vec![NumericComputationNode::literal(
                crate::SCALAR_INFO_ID.to_string(),
                None,
                &MathValue::integer(7),
            )
            .unwrap()],
            0,
        )
        .unwrap();
        let budget = NumericExecutionBudget {
            storage_bytes: 1_048_576,
            work_steps: 100_000,
            limbs: 128,
            symbolic_nodes: 128,
        };
        let requirement = NumericRequirement {
            semantics: NumericSemantics::Mathematical,
            representation: NumericRepresentation::Automatic,
            rounding: Binary64Rounding::NearestEven,
            accuracy: NumericAccuracy::Exact,
            parameters: Vec::new(),
            budget,
            reproducible: true,
            exceptions: Default::default(),
        };
        let offer = NumericRealizationOffer {
            algorithm: NumericAlgorithm::OrderedCheckedI64V1,
            budget,
        };
        (graph, requirement, offer)
    }
    #[test]
    fn offer_declaration_and_recomputed_witness_have_distinct_authority() {
        let (graph, requirement, mut offer) = fixture();
        offer.budget.limbs = 1;
        offer.budget.symbolic_nodes = 0;
        let declared = offer.configuration().unwrap();
        assert_eq!(
            NumericRealizationOffer::from_configuration(&declared).unwrap(),
            offer
        );
        assert!(verify_numeric_witness_configuration(&declared, &graph, &requirement).is_err());
        let witness = offer.certify(&graph, &requirement).unwrap();
        let reservation = witness.required_memory_reservation_bytes(&graph).unwrap();
        assert!(reservation >= witness.claim().storage_bytes);
        assert!(
            reservation <= 65_536,
            "native offer must cover its full preparation ledger: {reservation}"
        );
        let sealed = numeric_witness_configuration(&witness).unwrap();
        assert_eq!(
            verify_numeric_witness_configuration(&sealed, &graph, &requirement).unwrap(),
            witness
        );
        let mut changed = requirement;
        changed.rounding = Binary64Rounding::TowardPositive;
        assert!(verify_numeric_witness_configuration(&sealed, &graph, &changed).is_err());
    }
    #[test]
    fn tensor_witness_binds_ordered_coordinates_and_shared_requirement() {
        let (first, requirement, offer) = fixture();
        let second = NumericComputation::new(
            vec![NumericComputationNode::literal(
                crate::SCALAR_INFO_ID.to_string(),
                None,
                &MathValue::integer(9),
            )
            .unwrap()],
            0,
        )
        .unwrap();
        let witnesses = vec![
            offer.certify(&first, &requirement).unwrap(),
            offer.certify(&second, &requirement).unwrap(),
        ];
        let sealed = numeric_tensor_witness_configuration(&witnesses).unwrap();
        assert_eq!(
            verify_numeric_tensor_witness_configuration(
                &sealed,
                &[first.clone(), second.clone()],
                &requirement
            )
            .unwrap(),
            witnesses
        );
        assert!(verify_numeric_tensor_witness_configuration(
            &sealed,
            &[second.clone(), first.clone()],
            &requirement
        )
        .is_err());
        assert!(numeric_tensor_witness_configuration(&[]).is_err());
        assert!(numeric_tensor_witness_configuration(&vec![witnesses[0].clone(); 9]).is_err());
        let mut changed = requirement;
        changed.rounding = Binary64Rounding::TowardNegative;
        assert!(
            verify_numeric_tensor_witness_configuration(&sealed, &[first, second], &changed)
                .is_err()
        );
    }
    #[test]
    fn tensor_coordinates_share_one_total_work_budget() {
        let (graph, mut requirement, offer) = fixture();
        let witness = offer.certify(&graph, &requirement).unwrap();
        requirement.budget.work_steps = witness.claim().work_steps;
        assert!(offer.certify(&graph, &requirement).is_ok());
        assert_eq!(
            offer
                .certify_tensor(&[graph.clone(), graph], &requirement)
                .unwrap_err(),
            NumericEngineRefusal::Arithmetic(NumericRefusal::WorkBudget)
        );
    }
    #[test]
    fn offered_work_and_storage_are_hard_eligibility_bounds() {
        let (graph, requirement, mut offer) = fixture();
        offer.budget.storage_bytes = 1;
        assert!(offer.certify(&graph, &requirement).is_err());
        offer.budget.storage_bytes = 1_048_576;
        let witness = offer.certify(&graph, &requirement).unwrap();
        assert!(witness.claim().work_steps > 0);
        offer.budget.work_steps = witness.claim().work_steps.saturating_sub(1).max(1);
        if witness.claim().work_steps > 1 {
            assert!(offer.certify(&graph, &requirement).is_err());
        }
    }
}
