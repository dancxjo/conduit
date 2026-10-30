//! CPU-only pins/authority/offers; the prepared portable software is retained.
use conduit_core as c;
use conduit_environment_make::DisplayOutputSource;
use conduit_tm1637_wire::{installation::*, owner::WriteAttachment, WriteContract};
pub(crate) struct NoPins;
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
use conduit_environment_runtime::{CaptureBack, PreparedCaptureSelector};
pub struct Fixture {
    pub host: c::HostAdvertisement,
    pub grant: c::AuthorityGrant,
    _pins: ReadyWriteBase<NoPins>,
    _software: Vec<CaptureBack>,
    _selectors: Vec<PreparedCaptureSelector>,
}
pub fn fixture(source: &DisplayOutputSource) -> Fixture {
    let mut host = c::HostAdvertisement {
        declared_name: None,
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
    let pins = ready(&host.host_id, &host.boot_id);
    pins.append_to_advertisement(&mut host, &WriteContract::prepare().unwrap())
        .unwrap();
    let mut software = Vec::new();
    let mut selectors = Vec::new();
    for (index, gear) in source
        .plot
        .expanded
        .gears
        .iter()
        .enumerate()
        .filter(|(_, gear)| gear.kind_id.as_str() != conduit_tm1637_wire::WRITE_KIND)
    {
        let selector = gear.configuration.iter().any(|c| c.key == "selector");
        let c::ConfigurationValue::Text(encoded) = &gear
            .configuration
            .iter()
            .find(|entry| entry.key == if selector { "selector" } else { "program" })
            .unwrap()
            .value
        else {
            panic!("checked portable program")
        };
        if selector {
            selectors.push(
                PreparedCaptureSelector::prepare(
                    c::StructuredSelector::from_canonical_hex(encoded).unwrap(),
                    1024,
                    1024,
                )
                .unwrap(),
            );
            software.push(CaptureBack::selector(1024));
        } else {
            software.push(
                CaptureBack::expression(
                    &conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).unwrap(),
                    c::PortTemporal::Flow { closes: true },
                )
                .unwrap(),
            );
        }
        let kind = c::Kind {
            kind_id: gear.kind_id.clone(),
            kind_contract_revision: gear.kind_contract_revision.clone(),
            startup_parameters: gear.startup_parameters.clone(),
            shorthand: gear.shorthand.clone(),
            inputs: gear.inputs.clone(),
            outputs: gear.outputs.clone(),
            configuration: gear.semantic_contract.configuration.clone(),
            semantic_laws: gear.semantic_contract.laws.clone(),
            limits: c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 1024,
            },
        };
        let offer = c::BackOfferBuilder::new(
            kind,
            c::Back {
                capability_id: format!("fixture/software/{index}").into(),
                execution_profile_id: "fixture/software".into(),
                implementation_id: format!("fixture/software/{index}").into(),
                artifact_id: "fixture/software".into(),
                host_calls: if selector {
                    vec![c::HostCallRequirement {
                        contract_id: "conduit.host/structured-selector@1".into(),
                        target_kind: Some(gear.kind_id.clone()),
                        maximum_in_flight: 1,
                        maximum_input_bytes: 1024,
                        maximum_output_bytes: 1024,
                    }]
                } else {
                    vec![]
                },
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build();
        host.capabilities.push(offer);
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
    Fixture {
        host,
        grant,
        _pins: pins,
        _software: software,
        _selectors: selectors,
    }
}

pub(crate) fn ready(host: &c::HostId, boot: &c::BootId) -> ReadyWriteBase<NoPins> {
    let identity = WriteNativeIdentity {
        host_id: host.clone(),
        boot_id: boot.clone(),
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
    unsafe {
        ReadyWriteBase::new(
            identity,
            WriteAttachment {
                generation: 1,
                resource_bytes: 64,
            },
            NoPins,
        )
    }
    .unwrap()
}
