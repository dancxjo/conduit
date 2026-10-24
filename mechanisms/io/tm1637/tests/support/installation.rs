//! Installed ownership is fixture-only; no physical pin is asserted here.
use super::*;
use conduit_tm1637_wire::installation::*;
pub(super) fn identity() -> WriteNativeIdentity {
    let scope = fixture::scope();
    WriteNativeIdentity {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_id: "base/display".into(),
        provider_instance_id: scope.base_instance_id,
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        artifact_id: "artifact/display/fixture".into(),
        implementation_id: scope.implementation_id,
        base_implementation_id: "base/display/fixture@1".into(),
    }
}
fn attachment() -> WriteAttachment {
    WriteAttachment {
        generation: 7,
        resource_bytes: 4096,
    }
}
pub(super) fn ready(state: Rc<State>) -> ReadyWriteBase<Provider> {
    // SAFETY: inert CPU pin fixture is retained and follows the tested drop law.
    unsafe { ReadyWriteBase::new(identity(), attachment(), Provider(state)) }.unwrap()
}
fn advertisement() -> HostAdvertisement {
    let identity = identity();
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: identity.host_id,
        boot_id: identity.boot_id,
        offer_generation: OfferGeneration(1),
        profile: "fixture/display".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
#[test]
fn only_retained_native_slot_projects_its_exact_resource_and_zero_result_offer() {
    let state = Rc::new(State::default());
    let ready = ready(state.clone());
    let mut host = advertisement();
    ready
        .append_to_advertisement(&mut host, &WriteContract::prepare().unwrap())
        .unwrap();
    assert_eq!(host.bases.len(), 1);
    assert_eq!(host.resources.len(), 1);
    assert_eq!(host.capabilities.len(), 1);
    let offer = &host.capabilities[0];
    assert_eq!(offer.kind_id.as_str(), WRITE_KIND);
    assert_eq!(offer.host_calls[0].contract_id.as_str(), WRITE_CALL);
    assert_eq!(offer.host_calls[0].maximum_input_bytes, MAXIMUM_BYTES);
    assert_eq!(offer.host_calls[0].maximum_output_bytes, 0);
    assert_eq!(
        offer.authority_requirements[0].contract_id.as_str(),
        WRITE_AUTHORITY
    );
    assert_eq!(
        host.bases[0].provider_instance_id,
        identity().provider_instance_id
    );
    let retained = host.clone();
    assert!(ready
        .append_to_advertisement(&mut host, &WriteContract::prepare().unwrap())
        .is_err());
    assert_eq!(host, retained);
    host.boot_id = "fixture/stale".into();
    let retained = host.clone();
    assert_eq!(
        ready.append_to_advertisement(&mut host, &WriteContract::prepare().unwrap()),
        Err(InstallationRefusal::WrongBoot)
    );
    assert_eq!(host, retained);
    assert_eq!(state.edges.get(), 0);
}
