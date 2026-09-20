use conduit_core::*;
use conduit_std_host::ros2_base::*;

const ADAPTER: &str = "adapter/ros2/fixture";

fn mapping(id: &str, topic: &str, direction: InteropDirection) -> InteropMapping {
    InteropMapping {
        mapping_id: InteropMappingId::from(id),
        adapter_id: InteropAdapterId::from(ADAPTER),
        base_instance_id: BaseInstanceId::from("base/ros2/topics"),
        external_resource_id: topic_resource(topic),
        semantic_kind: KindId::from("value/text@1"),
        semantic_revision: KindContractRevision::from("1"),
        direction,
        authority_grant_id: AuthorityGrantId::from(format!("grant/{id}")),
        maximum_payload_bytes: 68,
        maximum_queued_items: 4,
        external_delivery: ExternalDeliveryContract::AtLeastOnce,
        declared_semantic_delivery: ExternalDeliveryContract::AtLeastOnce,
        preserves_external_lifecycle: true,
        intentional_reimport_of: None,
    }
}

pub(crate) fn topic(id: &str, name: &str, direction: InteropDirection) -> RosTopicConfiguration {
    RosTopicConfiguration {
        mapping: mapping(id, name, direction),
        topic_name: name.into(),
        interface_type: ROS_STRING_TYPE.into(),
        qos: RosQos {
            reliability: RosReliability::Reliable,
            durability: RosDurability::Volatile,
            history_depth: 4,
        },
        origin_parameter: "conduit_origin".into(),
    }
}

pub(crate) fn authority(mapping_id: &str, operation: &str) -> RosTopicAuthority {
    let operation = HostOperationContractId::from(operation);
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/ros2"),
        boot_id: BootId::from("boot/current"),
        base_instance_id: BaseInstanceId::from("base/ros2/topics"),
        base_provider_generation: 1,
        plan_id: PlanId::from("plan/ros2-bridge"),
        active_play_id: ActivePlayId::from("play/ros2-bridge"),
        authority_grant_id: AuthorityGrantId::from(format!("grant/{mapping_id}")),
        authority_contract_id: AuthorityContractId::from("authority/ros2-topic@1"),
        capability_id: CapabilityId::from(format!("ros2/{mapping_id}")),
        implementation_id: ImplementationId::from("ros2/native-topic-base@1"),
        operation_contract_id: operation.clone(),
        subject_kind: KindId::from("value/text@1"),
        resource_pool_id: ResourcePoolId::from(mapping_id),
        resource_generation_id: ResourceGenerationId(format!("{mapping_id}/generation-1")),
        envelope_id: CapabilityEnvelopeId::from("ros2/string/max-68/queue-4"),
        maximum_parameter_bytes: 68,
        maximum_result_bytes: 68,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let issue = CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_operation_contract_id: operation.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: operation,
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 68,
            maximum_result_bytes: 68,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 2,
        },
        scope,
    };
    let scope = &issue.scope;
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 0,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [4; 32],
        1,
    )
    .unwrap();
    let handle = table.issue(issue).unwrap();
    RosTopicAuthority {
        mapping_id: InteropMappingId::from(mapping_id),
        table,
        handle,
        claim,
    }
}

#[derive(Default)]
struct Provider {
    messages: Vec<(String, Vec<u8>, String)>,
}

impl NativeRosTopicProvider for Provider {
    fn publish(
        &mut self,
        topic_name: &str,
        interface_type: &str,
        _qos: RosQos,
        encoded: &[u8],
        origin: &str,
    ) -> Result<(), RosBaseRefusal> {
        assert_eq!(interface_type, ROS_STRING_TYPE);
        self.messages
            .push((topic_name.into(), encoded.into(), origin.into()));
        Ok(())
    }
}

#[test]
fn selected_topics_map_both_directions_with_exact_bounds_and_qos() {
    let mut base = RosTopicBase::prepare(vec![
        topic(
            "input",
            "/fixture/input",
            InteropDirection::ExternalToConduit,
        ),
        topic(
            "output",
            "/fixture/output",
            InteropDirection::ConduitToExternal,
        ),
    ])
    .unwrap();
    assert!(matches!(
        base.consider_discovery("/fixture/input", 9, None),
        ImportDecision::Admitted { .. }
    ));
    assert_eq!(
        base.consider_discovery("/fixture/sibling", 9, None),
        ImportDecision::Unconfigured
    );
    let facts = base.inspections().collect::<Vec<_>>();
    assert_eq!(facts.len(), 2);
    assert_eq!(facts[0].qos.history_depth, 4);

    let mut subscriber = authority("input", "conduit.host/ros2-subscribe@1");
    let encoded = encode_ros_string("hello", 68).unwrap();
    assert_eq!(
        base.import_string(
            &InteropMappingId::from("input"),
            ROS_STRING_TYPE,
            &encoded,
            &mut subscriber
        )
        .unwrap(),
        "hello"
    );

    let mut publisher = authority("output", "conduit.host/ros2-publish@1");
    let mut provider = Provider::default();
    let manifestation = base
        .publish_string(
            &InteropMappingId::from("output"),
            "HELLO",
            &mut publisher,
            &mut provider,
        )
        .unwrap();
    assert_eq!(provider.messages.len(), 1);
    assert_eq!(provider.messages[0].0, "/fixture/output");
    assert_eq!(decode_ros_string(&provider.messages[0].1).unwrap(), "HELLO");
    assert_eq!(
        base.consider_discovery(
            "/fixture/output",
            provider.messages[0].1.len() as u32,
            Some(manifestation.origin)
        ),
        ImportDecision::Unconfigured
    );
}

#[test]
fn direction_sibling_type_capacity_revocation_and_lifecycle_fail_closed() {
    let mut base = RosTopicBase::prepare(vec![
        topic(
            "input",
            "/fixture/input",
            InteropDirection::ExternalToConduit,
        ),
        topic(
            "output",
            "/fixture/output",
            InteropDirection::ConduitToExternal,
        ),
    ])
    .unwrap();
    let encoded = encode_ros_string("hello", 68).unwrap();
    let mut wrong_direction = authority("output", "conduit.host/ros2-publish@1");
    assert_eq!(
        base.import_string(
            &InteropMappingId::from("input"),
            ROS_STRING_TYPE,
            &encoded,
            &mut wrong_direction
        ),
        Err(RosBaseRefusal::WrongDirection)
    );
    let mut input = authority("input", "conduit.host/ros2-subscribe@1");
    assert_eq!(
        base.import_string(
            &InteropMappingId::from("input"),
            "geometry_msgs/msg/Pose",
            &encoded,
            &mut input
        ),
        Err(RosBaseRefusal::WrongType)
    );
    input.table.revoke(&input.handle).unwrap();
    assert_eq!(
        base.import_string(
            &InteropMappingId::from("input"),
            ROS_STRING_TYPE,
            &encoded,
            &mut input
        ),
        Err(RosBaseRefusal::Capability(BaseCapabilityRefusal::Revoked))
    );
    base.set_active(false);
    assert_eq!(
        base.import_string(
            &InteropMappingId::from("input"),
            ROS_STRING_TYPE,
            &encoded,
            &mut input
        ),
        Err(RosBaseRefusal::Inactive)
    );
}
