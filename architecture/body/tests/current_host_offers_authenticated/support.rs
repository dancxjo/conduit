use conduit_core as c;
pub fn capability(size: usize) -> c::CapabilityOffer {
    let ty = c::StructuredInfoType::leaf(c::kind_id(c::TEXT_INFO_ID)).unwrap();
    let profile = ty.profile().unwrap().value_kind().clone();
    let value = c::StructuredInfoValue::leaf(ty, vec![b'x'; size]).unwrap();
    let default =
        c::StructuredConfigurationValue::new(profile.clone(), value.canonical_bytes().unwrap())
            .unwrap();
    c::BackOfferBuilder::new(
        c::Kind {
            startup_parameters: vec![c::FrontStartupParameter {
                name: "default".into(),
                value_type: profile.clone(),
                has_default: true,
            }],
            shorthand: None,
            kind_id: "fixture/immutable-default".into(),
            kind_contract_revision: "fixture/revision".into(),
            inputs: vec![],
            outputs: vec![],
            configuration: vec![c::KindConfigurationField {
                key: "default".into(),
                default_value: c::ConfigurationValue::Structured(default),
                rule: c::KindConfigurationRule::Structured { profile },
            }],
            semantic_laws: vec![],
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1,
            },
        },
        c::Back {
            capability_id: "fixture/back".into(),
            execution_profile_id: "fixture/profile".into(),
            implementation_id: "fixture/implementation".into(),
            artifact_id: "fixture/artifact".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}
