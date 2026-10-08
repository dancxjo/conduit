#![cfg(feature = "finite-value-owners")]
use conduit_core::*;
use conduitos::finite_value;
use conduitos::finite_value_owners::{
    flow_exactly_one::PreparedFlowExactlyOne, value_repeat::PreparedValueRepeat,
};

#[test]
fn target_offers_admit_exact_schema_and_refuse_foreign_realizations() {
    let value =
        CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
            .unwrap();
    let schema = StructuredInfoType::leaf(kind_id(SCALAR_INFO_ID)).unwrap();
    let repeat = finite_value::value_repeat_offer(&value, &schema).unwrap();
    PreparedValueRepeat::new(value.clone(), schema.clone(), repeat.clone()).unwrap();
    let capacity2 = finite_value::value_repeat_capacity2_offer(&value, &schema).unwrap();
    assert_ne!(repeat.capability_id, capacity2.capability_id);
    PreparedValueRepeat::new(value.clone(), schema.clone(), capacity2).unwrap();
    let singleton = finite_value::flow_exactly_one_offer(&value, &schema).unwrap();
    PreparedFlowExactlyOne::new(value.clone(), schema.clone(), singleton.clone()).unwrap();
    let mut forged = singleton;
    forged.limits.max_active_instances += 1;
    assert!(PreparedFlowExactlyOne::new(value.clone(), schema.clone(), forged).is_err());
    let mut foreign = repeat;
    foreign.implementation.artifact_id = "".into();
    assert!(PreparedValueRepeat::new(value, schema, foreign).is_err());
}
