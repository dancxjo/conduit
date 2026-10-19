//! Prepared consuming tolerance over every admitted physical evidence form.
use conduit_core::*;
use conduit_plot::{
    PortableExpressionOperation as Op, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator,
};
pub const PURE_NEAR_CONTRACT_STD_PROFILE: &str = "std/pure-expression-near-contract-hosted@1";
pub const NEAR_CONTRACT_BOUNDS_SCHEMA: &str = "std/near-contract-expression-bounds@1";
pub const NEAR_CONTRACT_REPLAY_WORK: u64 = 900_000_000;
const BOUND_FIELDS: [&str; 13] = [
    "maximum-constraints",
    "maximum-predicate-replay-work",
    "maximum-per-record-validation-work",
    "maximum-expression-nodes",
    "maximum-structured-nodes",
    "maximum-structured-depth",
    "prepared-replay-container-and-buffer-bytes",
    "retained-near-schema-bytes",
    "canonical-contract-bytes",
    "maximum-validated-input-bytes",
    "maximum-validated-output-bytes",
    "maximum-input-bytes",
    "output-capacity-bytes",
];
pub fn near_contract_bounds_profile() -> KindId {
    super::quantity_expression::bounds_property(
        NEAR_CONTRACT_BOUNDS_SCHEMA,
        &BOUND_FIELDS.map(|name| (name, 0)),
    )
    .expect("fixed Near bounds schema")
    .profile()
    .clone()
}

pub fn near_contract_requirements() -> MathematicalEnclosureRequirements {
    let mut work = ExactNumericWork::new(NEAR_CONTRACT_REPLAY_WORK);
    MathematicalEnclosureRequirements {
        maximum_width: ExactRational::ratio(1, 100_000_000_000_000_000, &mut work)
            .expect("fixed width"),
        maximum_leaf_width: ExactRational::ratio(1, 100_000_000_000_000_000_000, &mut work)
            .expect("fixed leaf width"),
        maximum_constant_terms: 32,
        maximum_root_refinements: 64,
        maximum_scratch_payload_bytes: 131072,
        maximum_retained_payload_bytes: 131072,
    }
}
pub fn has_near_contract(program: &PortableExpressionProgram) -> bool {
    let Op::ValueContract { input, output, .. } = &program.root.operation else {
        return false;
    };
    [input, output].into_iter().flatten().any(|c| {
        c.constraints
            .iter()
            .any(|c| matches!(c, ValueConstraint::NearQuantity { .. }))
    })
}
pub fn prepare_near_contract_expression(
    program: &PortableExpressionProgram,
) -> Result<PreparedPortableExpressionEvaluator, String> {
    PreparedPortableExpressionEvaluator::new_with_mathematical_near_contracts(
        program,
        near_contract_requirements(),
        Binary32Rounding::NearestTiesEven,
        NEAR_CONTRACT_REPLAY_WORK,
    )
    .map_err(|why| format!("Near contract preparation: {why:?}"))
}
/// Module-source identity; not an executable digest or execution attestation.
pub fn near_contract_std_artifact() -> ArtifactId {
    let digest = semantic_digest("std/near-contract-modules@1",concat!(
        include_str!("near_contract.rs"),
        include_str!("../../../src/installed_std/pure_expression_back.rs"),
        include_str!("../../../src/numerical_host/expressions.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/value_contract.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/predicate_bounds.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/structured.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/evaluate.rs"),
        include_str!("../../../../../architecture/core/src/value_constraint/mathematical_near.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/near_evidence.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/near_evidence/mathematical.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/mathematical_binary32.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/mathematical_binary32/projection.rs"),
        include_str!("../../../../../architecture/core/src/structured_info/borrowed.rs")
    ).as_bytes());
    let mut identity = String::from("conduit-plot/near-contract@1/");
    use core::fmt::Write;
    for byte in digest {
        write!(&mut identity, "{byte:02x}").expect("String write");
    }
    identity.into()
}
pub fn pure_near_contract_std_offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    if !has_near_contract(program) {
        return Err("Near contract profile requires qualified boundaries".into());
    }
    let evaluator = prepare_near_contract_expression(program)?;
    let bounds = evaluator
        .value_contract_bounds()
        .ok_or("Near contract bounds absent")?;
    let mut offer = super::pure_expression_std_offer(program, temporal)
        .map_err(|why| format!("Near contract Fore: {why:?}"))?;
    offer.capability_id = format!("std/near-contract/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_NEAR_CONTRACT_STD_PROFILE.into();
    offer.implementation.artifact_id = near_contract_std_artifact();
    let old = super::value_contract_bounds::pure_expression_value_contract_bounds_profile();
    offer.realization_properties.retain(|p| p.profile() != &old);
    // Each generic record validation has its own structural node ceiling and
    // each mathematical leaf has its own finite codec meter. Expose those
    // component bounds separately from predicate replay. This is not a total
    // expression CPU/WCET bound or a count of record-validation invocations.
    let nodes = MAXIMUM_STRUCTURED_INFO_NODES as u64;
    let per_record = nodes * MATHEMATICAL_VALUE_VALIDATION_WORK;
    let input = program
        .maximum_prepared_input_bytes()
        .map_err(|why| format!("Near input extent: {why:?}"))?;
    let output = u32::try_from(evaluator.output_capacity()).map_err(|_| "Near output extent")?;
    let values = [
        bounds.maximum_constraints,
        bounds.maximum_numerical_work,
        per_record,
        nodes,
        nodes,
        MAXIMUM_STRUCTURED_INFO_DEPTH as u64,
        bounds.prepared_near_replay_bytes,
        bounds.retained_near_schema_bytes,
        bounds.canonical_contract_bytes,
        bounds.maximum_input_bytes,
        bounds.maximum_output_bytes,
        input.into(),
        output.into(),
    ];
    let fields: Vec<_> = BOUND_FIELDS.into_iter().zip(values).collect();
    let property =
        super::quantity_expression::bounds_property(NEAR_CONTRACT_BOUNDS_SCHEMA, &fields)?;
    offer.realization_properties.push(property);
    offer
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    offer.limits.max_queue_bytes = offer.limits.max_queue_bytes.max(input).max(output);
    for call in &mut offer.host_calls {
        call.maximum_input_bytes = input;
        call.maximum_output_bytes = output;
    }
    Ok(offer)
}
