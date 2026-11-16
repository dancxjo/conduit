//! Explicit typed temporal lift; definitions confer no provider or authority.
use conduit_core::*;
pub const FLOW_SINGLETON_IMPLEMENTATION: &str = "std/flow-singleton@1";
pub fn flow_singleton_std_offer(
    value: &CheckedValueContract,
    schema: &StructuredInfoType,
) -> Result<CapabilityOffer, &'static str> {
    let contract = conduit_semantic_catalog::flow_singleton_semantic_contract(value, schema)?;
    let digest = semantic_digest(
        "std/flow-singleton@1",
        concat!(
            include_str!("flow_singleton.rs"),
            include_str!("../../../../semantics/data/src/flow_singleton_step.rs"),
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: FLOW_SINGLETON_IMPLEMENTATION.into(),
            execution_profile_id: "std/flow-singleton-hosted@1".into(),
            implementation_id: FLOW_SINGLETON_IMPLEMENTATION.into(),
            artifact_id: artifact.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}
/// The finite checked CPU inventory currently selects this exact schema only.
/// Other typed definitions do not silently become installed Ready providers.
pub fn measurement_series_singleton_std_offer() -> CapabilityOffer {
    let (schema, maximum) =
        conduit_data::measurement_chart_constructor_input(1).expect("bounded series schema");
    let value = CheckedValueContract::new(
        schema
            .profile()
            .expect("series profile")
            .value_kind()
            .clone(),
        maximum,
        vec![],
    )
    .expect("bounded series");
    flow_singleton_std_offer(&value, &schema).expect("explicit series singleton")
}
