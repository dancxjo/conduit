//! Exact schema specialization reuses the original prepared latest-value Back.
use conduit_core::{CapabilityOffer, CheckedValueContract};
pub fn combine_latest_schema_offer(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
) -> Result<CapabilityOffer, &'static str> {
    combine_latest_schema_offer_with_input_terminals(left, right, [true, true])
}
pub fn combine_latest_schema_offer_with_input_terminals(
    left: &CheckedValueContract,
    right: &CheckedValueContract,
    input_terminals: [bool; 2],
) -> Result<CapabilityOffer, &'static str> {
    let mut offer = super::combine_latest_offer(left, right)?;
    let contract = conduit_semantic_catalog::combine_latest_schema_with_input_terminals(
        left,
        right,
        input_terminals,
    )?;
    offer.implementation.implementation_id = format!(
        "std/kernel-combine-latest-schema-{}@1",
        contract
            .kind_id
            .as_str()
            .strip_prefix("state/combine-latest/schema-")
            .ok_or("exact latest schema identity")?
    )
    .into();
    offer.inputs = contract.inputs.clone();
    offer.outputs = contract.outputs.clone();
    // Several exact terminal policies may coexist on one Host.
    if input_terminals != [true, true] {
        offer.capability_id = format!(
            "{}-{}{}",
            offer.capability_id.as_str(),
            u8::from(input_terminals[0]),
            u8::from(input_terminals[1])
        )
        .into();
    }
    offer.kind_id = contract.kind_id.clone();
    offer.kind_contract_revision = contract.kind_contract_revision.clone();
    offer.semantic_contract = contract.semantic_contract();
    Ok(offer)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_realization_and_bounds_but_exact_specialized_contract() {
        let value = CheckedValueContract::new("test/value".into(), 128, vec![]).unwrap();
        let base = super::super::combine_latest_offer(&value, &value).unwrap();
        let exact = combine_latest_schema_offer(&value, &value).unwrap();
        assert_eq!(
            exact.implementation.artifact_id,
            base.implementation.artifact_id
        );
        assert_ne!(
            exact.implementation.implementation_id,
            base.implementation.implementation_id
        );
        assert_eq!(exact.host_calls, base.host_calls);
        assert_eq!(exact.limits, base.limits);
        assert_eq!(exact.inputs, base.inputs);
        assert_eq!(exact.outputs, base.outputs);
        assert_ne!(exact.kind_id, base.kind_id);
    }
}
