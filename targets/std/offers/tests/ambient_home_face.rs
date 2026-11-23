use conduit_core::{FACE_RESOURCE_CLASS, RUNTIME_MEMORY_RESOURCE_CLASS};
use conduit_semantic_catalog::{
    ambient_home_face_kind, ambient_home_workspace_bytes, AMBIENT_HOME_OBSERVATION_BYTES,
};
use conduit_std_offers::{
    resident_ambient_home_face_offer, AMBIENT_HOME_CUSTODY_CALL, AMBIENT_HOME_PROJECT_CALL,
};

#[test]
fn resident_home_face_offer_binds_both_operations_and_workspace() {
    let kind = ambient_home_face_kind().unwrap();
    let offer = resident_ambient_home_face_offer().unwrap();
    assert_eq!(offer.semantic_contract, kind.semantic_contract());
    for (call, identity) in offer
        .host_calls
        .iter()
        .zip([AMBIENT_HOME_PROJECT_CALL, AMBIENT_HOME_CUSTODY_CALL])
    {
        assert_eq!(call.contract_id.as_str(), identity);
        assert_eq!(call.target_kind.as_ref(), Some(&kind.kind_id));
        assert_eq!(
            (
                call.maximum_input_bytes,
                call.maximum_output_bytes,
                call.maximum_in_flight
            ),
            (AMBIENT_HOME_OBSERVATION_BYTES, 0, 1)
        );
    }
    assert_eq!(offer.host_calls.len(), 2);
    assert!(offer
        .resource_requirements
        .iter()
        .any(|resource| resource.class_id.as_str() == FACE_RESOURCE_CLASS && resource.units == 1));
    assert!(offer
        .resource_requirements
        .iter()
        .any(
            |resource| resource.class_id.as_str() == RUNTIME_MEMORY_RESOURCE_CLASS
                && resource.units == ambient_home_workspace_bytes().unwrap()
        ));
}
