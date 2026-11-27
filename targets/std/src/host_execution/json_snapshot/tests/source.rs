//! Sealed authored connectivity and exact canonical constructor admission.
use super::*;

#[test]
fn authored_publication_is_the_only_reference_update_path_into_body_state() {
    let root = tempfile::tempdir().unwrap();
    let (_, write, _) = fixture(root.path(), "boot/source-write", Mode::Publish);
    let (_, read, _) = fixture(root.path(), "boot/source-read", Mode::Recover);
    let input_fore = write
        .fore_ports
        .iter()
        .filter(|port| port.direction == PortDirection::Input)
        .collect::<Vec<_>>();
    assert_eq!(
        input_fore.len(),
        1,
        "there is no external reference/State update Fore"
    );
    assert_eq!(input_fore[0].front_port_id.as_str(), "json");
    assert_eq!(input_fore[0].track, ConnectionTrack::Payload);
    assert_eq!(
        input_fore[0].value_kind.as_str(),
        conduit_web::JSON_TEXT_INFO_ID
    );
    assert!(read
        .fore_ports
        .iter()
        .all(|port| port.direction != PortDirection::Input));
    let publish = write
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_PUBLISH_KIND)
        .unwrap();
    assert_eq!(input_fore[0].placement_id, publish.placement_id);
    assert_eq!(input_fore[0].gear_port_id, publish.inputs[0].port_id);
    let state = write
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_semantic_catalog::state_value::STATE_VALUE_KIND)
        .unwrap();
    let constructors = write
        .placements
        .iter()
        .filter(|placement| {
            placement
                .configuration
                .iter()
                .any(|entry| entry.key == "program")
        })
        .collect::<Vec<_>>();
    assert_eq!(constructors.len(), 1);
    let constructor = constructors[0];
    let payload = write
        .connections
        .iter()
        .filter(|cord| cord.track == ConnectionTrack::Payload)
        .collect::<Vec<_>>();
    assert_eq!(
        payload.len(),
        2,
        "exactly publication -> constructor -> State"
    );
    let publication_cord = payload
        .iter()
        .find(|cord| cord.source_placement_id == publish.placement_id)
        .unwrap();
    assert_eq!(publication_cord.source_port_id, publish.outputs[0].port_id);
    assert_eq!(publication_cord.sink_placement_id, constructor.placement_id);
    assert_eq!(publication_cord.sink_port_id, constructor.inputs[0].port_id);
    assert_eq!(
        publication_cord.value_kind.as_str(),
        RESOURCE_REFERENCE_INFO_ID
    );
    let state_inputs = write
        .connections
        .iter()
        .filter(|cord| cord.sink_placement_id == state.placement_id)
        .collect::<Vec<_>>();
    assert_eq!(
        state_inputs.len(),
        1,
        "no alternate route may update this State"
    );
    assert_eq!(state_inputs[0].track, ConnectionTrack::Payload);
    assert_eq!(
        state_inputs[0].source_placement_id,
        constructor.placement_id
    );
    assert_eq!(
        state_inputs[0].source_port_id,
        constructor.outputs[0].port_id
    );
    assert_eq!(state_inputs[0].sink_port_id.as_str(), "next");
    let ConfigurationValue::Text(code) = &constructor
        .configuration
        .iter()
        .find(|entry| entry.key == "program")
        .unwrap()
        .value
    else {
        panic!("ordinary sealed expression program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(code).unwrap();
    let reference_type = StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap();
    assert_eq!(program.input_type, reference_type);
    assert_eq!(program.output_type, optional().0);
    assert_eq!(
        state_inputs[0].value_kind,
        program.output_type.profile().unwrap().value_kind().clone()
    );
    let mut altered = reference();
    altered.lifetime.version = ResourceVersionIdentity::from_digest([77; 32]);
    let encoded = altered.encode().unwrap();
    assert_ne!(encoded, reference().encode().unwrap());
    let mut evaluator = conduit_plot::PreparedPortableExpressionEvaluator::new(&program).unwrap();
    let result = evaluator.evaluate(&encoded).unwrap();
    let result = StructuredInfoValue::from_canonical_bytes(result).unwrap();
    assert_eq!(result.value_type(), &optional().0);
    let StructuredInfoValueShape::Variant {
        tag: "some",
        payload,
    } = result.shape()
    else {
        panic!("ordinary some output")
    };
    assert_eq!(payload.value_type(), &reference_type);
    let StructuredInfoValueShape::Leaf(bytes) = payload.shape() else {
        panic!("opaque reference payload")
    };
    assert_eq!(bytes, encoded);
    assert_eq!(BoundedResourceRef::decode(bytes).unwrap(), altered);
    assert_eq!(write.states.len(), 1);
    assert_eq!(read.states.len(), 1);
    let (written, recovered) = (&write.states[0], &read.states[0]);
    assert_eq!(written.state_id, recovered.state_id);
    assert_eq!(written.value_kind, recovered.value_kind);
    assert_eq!(written.initial_value, recovered.initial_value);
    assert_eq!(written.maximum_value_bytes, recovered.maximum_value_bytes);
    assert_eq!(written.lifetime, StateLifetime::Body);
}
