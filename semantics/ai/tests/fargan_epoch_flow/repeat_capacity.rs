use conduit_composite::KernelOperationFactory;
use conduit_core::*;
use conduit_std_host::value_repeat::{PreparedValueRepeat, ValueRepeatOperationFactory};
use std::sync::Arc;

fn plan(
    capacity2: bool,
    count: usize,
) -> Result<(Plan, Arc<PreparedValueRepeat>), conduit_planner::PlannerError> {
    let schema = StructuredInfoType::leaf(kind_id(SCALAR_INFO_ID)).unwrap();
    let value =
        CheckedValueContract::new(kind_id(SCALAR_INFO_ID), SCALAR_ENCODED_LEN as u32, vec![])
            .unwrap();
    let profile = Arc::new(
        if capacity2 {
            PreparedValueRepeat::capacity2(value.clone(), schema.clone())
        } else {
            PreparedValueRepeat::new(value.clone(), schema.clone())
        }
        .unwrap(),
    );
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut types = conduit_plot::ProfileCatalog::new();
    let install = if capacity2 {
        conduit_semantic_catalog::install_value_repeat_capacity2_kind
    } else {
        conduit_semantic_catalog::install_value_repeat_kind
    };
    install(&value, &schema, &mut startup, &mut types).unwrap();
    let mut capabilities = vec![profile.offer().clone()];
    for (name, input) in std::iter::once(("source".into(), false))
        .chain((0..count).map(|index| (format!("sink{index}"), true)))
    {
        let kind_name = format!("test/repeat-capacity/{name}");
        let port = PortDescriptor {
            port_id: port_id("value"),
            value_kind: value.value_kind.clone(),
            direction: if input {
                PortDirection::Input
            } else {
                PortDirection::Output
            },
            temporal: if input {
                PortTemporal::Flow { closes: true }
            } else {
                PortTemporal::Value
            },
            abnormal_kind: None,
        };
        let kind = Kind {
            kind_id: kind_id(&kind_name),
            kind_contract_revision: "test/repeat-capacity@1".into(),
            startup_parameters: vec![],
            shorthand: None,
            inputs: if input { vec![port.clone()] } else { vec![] },
            outputs: if input { vec![] } else { vec![port.clone()] },
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![FrontValueContract {
                location: if input {
                    FrontValueLocation::Input(port.port_id)
                } else {
                    FrontValueLocation::Output(port.port_id)
                },
                contract: value.clone(),
            }])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: value.maximum_bytes,
            },
        };
        startup
            .insert(conduit_plot::KindSignature {
                kind: kind_name.clone(),
                startup_parameters: vec![],
            })
            .unwrap();
        types.insert_kind(kind.clone()).unwrap();
        capabilities.push(
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: kind_name.clone().into(),
                    execution_profile_id: "test/repeat-capacity@1".into(),
                    implementation_id: kind_name.clone().into(),
                    artifact_id: kind_name.into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
    }
    let mut source = "plot repeat-capacity {\n source: test/repeat-capacity/source\n".to_owned();
    for index in 0..count {
        source.push_str(&format!(" repeat{index}: {} (count = 1)\n sink{index}: test/repeat-capacity/sink{index}\n source.value >> repeat{index}.value\n repeat{index}.items >> sink{index}.value\n",profile.offer().kind_id.as_str()));
    }
    source.push_str("}\n");
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(&source),
        &startup,
    )
    .unwrap();
    let expanded =
        conduit_plot::expand_canonical_plot(&checked, "repeat-capacity", &types).unwrap();
    let hosts = [HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "repeat-capacity".into(),
        boot_id: "repeat-capacity-boot".into(),
        offer_generation: OfferGeneration(1),
        profile: "test/repeat-capacity@1".into(),
        bases: vec![],
        resources: vec![],
        planner_capabilities: vec![],
        capabilities,
    }];
    let choices = conduit_planner::default_expanded_placements(&expanded, &hosts)?;
    Ok((
        conduit_planner::plan_expanded_canonical(
            &expanded,
            &hosts,
            &choices,
            &["conduit.base/local@1".into()],
        )?,
        profile,
    ))
}

#[test]
fn explicit_two_instance_repeat_preserves_default_and_refuses_overbooking_or_drift() {
    assert!(matches!(
        plan(false, 2),
        Err(conduit_planner::PlannerError::UnknownCapability(_))
    ));
    assert!(matches!(
        plan(true, 3),
        Err(conduit_planner::PlannerError::UnknownCapability(_))
    ));
    let (plan, profile) = plan(true, 2).unwrap();
    assert_eq!(profile.offer().limits.max_active_instances, 2);
    let factory =
        ValueRepeatOperationFactory::for_plan(&plan, std::slice::from_ref(&profile)).unwrap();
    let mut store = conduit_kernel::HostedValueStore::new(16, 4096, 65536).unwrap();
    let gears: Vec<_> = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .filter(|gear| {
            gear.implementation_id.as_str() == conduit_std_offers::VALUE_REPEAT_IMPLEMENTATION
        })
        .collect();
    assert_eq!(gears.len(), 2);
    for gear in gears {
        factory.prepare(gear, &mut store).unwrap();
        let mut changed = gear.clone();
        changed.limits.max_active_instances = 1;
        assert!(factory.budget(&changed).is_err());
        assert!(factory.prepare(&changed, &mut store).is_err());
    }
}
