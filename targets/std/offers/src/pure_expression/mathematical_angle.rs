//! Explicit hosted Angle-frame transport/trig with recomputable finite bounds.
use conduit_core::{
    ArtifactId, CapabilityOffer, PortTemporal, QuantityValueProfile, QuantityWireContract,
    StructuredInfoType, StructuredInfoTypeShape,
};
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};
pub const PURE_MATHEMATICAL_ANGLE_STD_PROFILE: &str =
    "std/pure-expression-mathematical-angle-hosted@1";
pub const MATHEMATICAL_ANGLE_BOUNDS_SCHEMA: &str = "std/mathematical-angle-expression-bounds@1";
/// Bind the selected Angle computation and installed admission code. This is a
/// module-source artifact identity, not an executable-binary or live-run receipt.
pub fn mathematical_angle_std_artifact() -> ArtifactId {
    let digest=conduit_core::semantic_digest("std/mathematical-angle-modules@1",concat!(
        include_str!("mathematical_angle.rs"),
        include_str!("quantity_expression.rs"),
        include_str!("../../../src/installed_std/pure_expression_back.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/prepare.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/evaluate.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/quantity_profile.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/angle_reference.rs"),
        include_str!("../../../../../architecture/plot/src/expression_prepared/trigonometry.rs"),
        include_str!("../../../../../architecture/plot/src/expression_angle_reference.rs"),
        include_str!("../../../../../architecture/plot/src/expression_trigonometry.rs"),
        include_str!("../../../../../architecture/plot/src/expression_semantic_call.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/angle_reference.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/mathematical.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/mathematical_trigonometry.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/value_profile.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/value_view.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/quantity/wire_contract.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/symbolic/codec.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/symbolic/pi_coefficient.rs"),
        include_str!("../../../../../architecture/core/src/exact_numeric/symbolic/trigonometry.rs")
    ).as_bytes());
    let mut identity = String::from("conduit-plot/mathematical-angle@1/");
    use core::fmt::Write;
    for b in digest {
        write!(&mut identity, "{b:02x}").expect("String write");
    }
    identity.into()
}
pub fn pure_mathematical_angle_std_offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, String> {
    if temporal != PortTemporal::Value {
        return Err("mathematical Angle requires one Value transaction".into());
    }
    let evaluator = PreparedPortableExpressionEvaluator::new_with_mathematical_angles(program)
        .map_err(|why| format!("mathematical Angle preparation: {why:?}"))?;
    let (nodes, depth, payload, work) = evaluator
        .mathematical_angle_profile_bounds()
        .ok_or("mathematical Angle prepared bounds unavailable")?;
    let mut offer = super::pure_expression_std_offer(program, temporal)
        .map_err(|why| format!("mathematical Angle Fore: {why:?}"))?;
    offer.capability_id = format!("std/mathematical-angle/{}", offer.kind_id.as_str()).into();
    offer.implementation.execution_profile_id = PURE_MATHEMATICAL_ANGLE_STD_PROFILE.into();
    offer.implementation.artifact_id = mathematical_angle_std_artifact();
    let profile = QuantityValueProfile::Mathematical;
    let inputs = if angle(&program.input_type) {
        vec![("input".into(), profile)]
    } else {
        vec![]
    };
    let outputs = if angle(&program.output_type) {
        vec![("output".into(), profile)]
    } else {
        vec![]
    };
    let wire = QuantityWireContract::new(inputs, outputs)
        .map_err(|why| format!("mathematical Angle ports: {why:?}"))?
        .to_property()
        .map_err(|why| format!("mathematical Angle wire: {why:?}"))?;
    let input = program.input_maximum_bytes.map_or_else(
        || {
            if angle(&program.input_type) {
                Ok(profile.maximum_encoded_bytes() as u32)
            } else {
                program
                    .maximum_prepared_input_bytes()
                    .map_err(|why| format!("mathematical input: {why:?}"))
            }
        },
        Ok,
    )?;
    let output =
        u32::try_from(evaluator.output_capacity()).map_err(|_| "mathematical output extent")?;
    let bounds = super::quantity_expression::bounds_property(
        MATHEMATICAL_ANGLE_BOUNDS_SCHEMA,
        &[
            ("maximum-nodes", nodes as u64),
            ("maximum-depth", depth as u64),
            ("retained-prepared-payload-ceiling-bytes", payload as u64),
            ("maximum-numerical-work", work),
            ("maximum-input-bytes", u64::from(input)),
            ("output-capacity-bytes", u64::from(output)),
        ],
    )?;
    offer.limits.max_queue_bytes = offer.limits.max_queue_bytes.max(input).max(output);
    for call in &mut offer.host_calls {
        call.maximum_input_bytes = input;
        call.maximum_output_bytes = output;
    }
    offer.realization_properties.extend([wire, bounds]);
    offer
        .realization_properties
        .sort_by(|a, b| a.profile().cmp(b.profile()));
    Ok(offer)
}
fn angle(ty: &StructuredInfoType) -> bool {
    matches!(ty.shape(),StructuredInfoTypeShape::Leaf(kind) if kind.as_str()==conduit_core::ANGLE_INFO_ID)
}
