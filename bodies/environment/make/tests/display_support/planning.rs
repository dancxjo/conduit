//! Initialized pin/authority and software descriptors are explicit CPU fixtures.
use conduit_core as c;
use conduit_environment_make::{
    DisplayOutputSource, DisplayPlanningRefusal, PreparedDisplayOutput,
};
use conduit_tm1637_wire::{installation::*, owner::WriteAttachment, WriteContract};
struct NoPins;
impl conduit_tm1637_wire::OpenDrain for NoPins {
    type Error = core::convert::Infallible;
    fn clock(&mut self, _: bool) -> Result<(), Self::Error> {
        panic!("planning has no effects")
    }
    fn data(&mut self, _: bool) -> Result<(), Self::Error> {
        panic!("planning has no effects")
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        panic!("planning has no effects")
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        panic!("planning has no effects")
    }
}
fn fixture(source: &DisplayOutputSource) -> (c::HostAdvertisement, c::AuthorityGrant) {
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/display".into(),
        boot_id: "fixture/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/display".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let identity = WriteNativeIdentity {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        base_id: "fixture/pins".into(),
        provider_instance_id: "fixture/provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pins".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/driver".into(),
        implementation_id: "fixture/driver".into(),
        base_implementation_id: "fixture/base".into(),
    };
    // SAFETY: inert owned fixture; any attempted electrical operation panics.
    let ready = unsafe {
        ReadyWriteBase::new(
            identity,
            WriteAttachment {
                generation: 1,
                resource_bytes: 64,
            },
            NoPins,
        )
    }
    .unwrap();
    ready
        .append_to_advertisement(&mut host, &WriteContract::prepare().unwrap())
        .unwrap();
    for (index, gear) in source.plot.expanded.gears.iter().enumerate() {
        if gear.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND {
            continue;
        }
        let temporal = c::PortTemporal::Flow { closes: true };
        let kind = if let Some(c::ConfigurationValue::Text(program)) = gear
            .configuration
            .iter()
            .find(|entry| entry.key == "program")
            .map(|entry| &entry.value)
        {
            conduit_semantic_catalog::pure_expression_contract(
                &conduit_plot::PortableExpressionProgram::from_canonical_hex(program).unwrap(),
                temporal,
            )
            .unwrap()
        } else {
            let c::ConfigurationValue::Text(selector) = &gear
                .configuration
                .iter()
                .find(|entry| entry.key == "selector")
                .unwrap()
                .value
            else {
                panic!("selector")
            };
            conduit_semantic_catalog::structured_selector_contract(
                &c::StructuredSelector::from_canonical_hex(selector).unwrap(),
                temporal,
            )
        };
        host.capabilities.push(
            c::BackOfferBuilder::new(
                kind,
                c::Back {
                    capability_id: format!("fixture/software/{index}").into(),
                    execution_profile_id: "fixture/software".into(),
                    implementation_id: format!("fixture/software/{index}").into(),
                    artifact_id: "fixture/software".into(),
                    host_calls: vec![],
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build(),
        );
    }
    let grant = c::AuthorityGrant {
        grant_id: "fixture/display".into(),
        contract_id: WRITE_AUTHORITY.into(),
        host_call_contract_id: conduit_tm1637_wire::WRITE_CALL.into(),
        subject_kind: c::kind_id(conduit_tm1637_wire::WRITE_KIND),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: WRITE_CAPABILITY.into(),
    };
    (host, grant)
}
#[test]
fn ordinary_output_planning_requires_current_output_authority_and_seals_both_fore() {
    let source = DisplayOutputSource::compile().unwrap();
    let (host, grant) = fixture(&source);
    let bases = [
        c::BaseImplementationId::from("fixture/base"),
        c::BaseImplementationId::from("conduit.base/local@1"),
    ];
    assert!(matches!(
        PreparedDisplayOutput::prepare(&source, &host, &bases, &[]),
        Err(DisplayPlanningRefusal::Planning(
            conduit_planner::PlannerError::AuthorityGrantMissing(_)
        ))
    ));
    let mut foreign = grant.clone();
    foreign.boot_id = "fixture/foreign-boot".into();
    assert!(PreparedDisplayOutput::prepare(&source, &host, &bases, &[foreign]).is_err());
    let mut missing = host.clone();
    missing
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND);
    assert!(matches!(
        PreparedDisplayOutput::prepare(&source, &missing, &bases, core::slice::from_ref(&grant)),
        Err(DisplayPlanningRefusal::Placement(_))
    ));
    assert!(
        PreparedDisplayOutput::prepare(&source, &host, &[], core::slice::from_ref(&grant)).is_err()
    );
    let prepared = PreparedDisplayOutput::prepare(&source, &host, &bases, &[grant]).unwrap();
    let fragment = &prepared.plan().fragments[0];
    assert!(c::verify_plan_fragment(fragment));
    assert_eq!(fragment.fore_ports.len(), 2);
    for (name, direction, bytes) in [
        (
            "frame",
            c::PortDirection::Input,
            conduit_tm1637_wire::MAXIMUM_BYTES,
        ),
        ("invalid_brightness", c::PortDirection::Output, 64),
    ] {
        let port = fragment
            .fore_ports
            .iter()
            .find(|port| port.front_port_id.as_str() == name)
            .unwrap();
        assert_eq!(port.direction, direction);
        assert_eq!(port.temporal, c::PortTemporal::Flow { closes: true });
        assert_eq!((port.item_capacity, port.byte_capacity), (1, bytes));
    }
    assert_eq!(
        prepared.lowered().nodes.len(),
        source.plot.expanded.gears.len()
    );
    assert_eq!(
        fragment
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_tm1637_wire::WRITE_KIND)
            .count(),
        1
    );
}
