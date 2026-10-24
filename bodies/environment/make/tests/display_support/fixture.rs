//! Pins and authority are CPU fixtures; portable software owners are real.
use conduit_core as c;
use conduit_environment_make::DisplayOutputSource;
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
use conduit_environment_runtime::appliance::software::ReadySoftwareBacks;
pub(crate) fn fixture(
    source: &DisplayOutputSource,
) -> (c::HostAdvertisement, c::AuthorityGrant, ReadySoftwareBacks) {
    fixture_with_boot(source, "fixture/boot")
}
pub(crate) fn fixture_with_boot(
    source: &DisplayOutputSource,
    boot: &str,
) -> (c::HostAdvertisement, c::AuthorityGrant, ReadySoftwareBacks) {
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "fixture/display".into(),
        boot_id: boot.into(),
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
    let mut software = source
        .prepare_software(host.host_id.clone(), host.boot_id.clone())
        .unwrap();
    software.append_to_advertisement(&mut host).unwrap();
    let grant = c::AuthorityGrant {
        grant_id: "fixture/display".into(),
        contract_id: WRITE_AUTHORITY.into(),
        host_call_contract_id: conduit_tm1637_wire::WRITE_CALL.into(),
        subject_kind: c::kind_id(conduit_tm1637_wire::WRITE_KIND),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: WRITE_CAPABILITY.into(),
    };
    (host, grant, software)
}
