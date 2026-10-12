//! Delayed native results must fit the exact prepared FeedbackZip envelope.
use super::*;

#[test]
fn ecm_accept_admits_the_actual_feedback_pair_encoder_envelope() {
    let prepared = source();
    let expanded = prepared.expand("cdc-ecm-frame-service").unwrap();
    let zip = expanded
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "flow/zip/feedback")
        .unwrap();
    let zip_front = zip.checked_front();
    assert_eq!(zip_front.inputs().len(), 2);
    let member_limits: Vec<_> = zip_front
        .inputs()
        .iter()
        .map(|port| {
            zip_front
                .value_contract(&FrontValueLocation::Input(port.port_id.clone()))
                .unwrap()
                .maximum_bytes
        })
        .collect();
    assert_eq!(member_limits, [4096, 4096]);
    let acceptance = program(&prepared, "cdc-ecm-accept");
    let encoder = PreparedTypedTuplePairEncoder::new(
        field(&acceptance.input_type, "0").clone(),
        member_limits[0],
        field(&acceptance.input_type, "1").clone(),
        member_limits[1],
    )
    .unwrap();
    assert_eq!(encoder.value_type(), &acceptance.input_type);
    // Derive this from the real encoder, not a duplicate framing formula or
    // the payload-only sum which originally underbounded the native cord.
    let expected = encoder.maximum_bytes();
    assert!(expected > member_limits.iter().sum::<u32>());
    let pair_output = zip_front
        .value_contract(&FrontValueLocation::Output(
            zip_front.outputs()[0].port_id.clone(),
        ))
        .unwrap();
    assert_eq!(pair_output.maximum_bytes, expected);

    let accept = prepared.expand("cdc-ecm-accept").unwrap();
    let front = &accept.front;
    assert_eq!(
        front
            .value_contract(&FrontValueLocation::Input(
                front.inputs()[0].port_id.clone(),
            ))
            .unwrap()
            .maximum_bytes,
        expected
    );
    assert_eq!(
        front
            .value_contract(&FrontValueLocation::Output(
                front.outputs()[0].port_id.clone(),
            ))
            .unwrap()
            .maximum_bytes,
        8192
    );
    // The following state transition retains its original contract. Increasing
    // a pair's complete canonical envelope grants no USB/network capacity.
    let advance = prepared.expand("cdc-ecm-advance").unwrap();
    assert_eq!(
        advance
            .front
            .value_contract(&FrontValueLocation::Input(
                advance.front.inputs()[0].port_id.clone(),
            ))
            .unwrap()
            .maximum_bytes,
        8192
    );
    assert_eq!(
        advance
            .front
            .value_contract(&FrontValueLocation::Output(
                advance.front.outputs()[0].port_id.clone(),
            ))
            .unwrap()
            .maximum_bytes,
        4096
    );
}
