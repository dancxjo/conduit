use conduit_core::{ArtifactId, CapabilityId, HostId, ImplementationId, KindContractRevision};
use conduit_form::parse;
use conduit_signal::{pico_local_advertisement, signal_profile_catalog, PULSE_KIND};

#[allow(dead_code)]
pub fn pulse_operation() -> conduit_form::CheckedOperation {
    parse(
        "form 0\n\nrealization {\n    pulse: flow/pulse\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n}\n",
        &signal_profile_catalog(),
    )
    .expect("pulse form checks")
    .operations
    .remove(0)
}

pub fn competing_hosts() -> [conduit_core::HostAdvertisement; 2] {
    let source = pico_local_advertisement();
    let pulse = source
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == PULSE_KIND)
        .expect("pulse offer exists");

    let mut efficient_offer = pulse.clone();
    efficient_offer.capability_id = CapabilityId::from("efficient/pulse");
    efficient_offer.implementation.implementation_id = ImplementationId::from("efficient/pulse@1");
    efficient_offer.implementation.artifact_id = ArtifactId::from("efficient/pulse-artifact@1");
    efficient_offer.limits.max_queue_items = 4;
    efficient_offer.limits.max_active_instances = 2;
    efficient_offer.resource_requirements[0].units = 1;

    let mut capable_offer = pulse.clone();
    capable_offer.capability_id = CapabilityId::from("capable/pulse");
    capable_offer.kind_id = conduit_core::kind_id("alternate/nominal-pulse");
    capable_offer.kind_contract_revision = KindContractRevision::from("alternate/nominal-pulse@9");
    capable_offer.implementation.implementation_id = ImplementationId::from("capable/pulse@1");
    capable_offer.implementation.artifact_id = ArtifactId::from("capable/pulse-artifact@1");
    capable_offer.limits.max_queue_items = 8;
    capable_offer.limits.max_active_instances = 2;
    capable_offer.resource_requirements[0].units = 3;

    let mut efficient = source.clone();
    efficient.host_id = HostId::from("host-a-efficient");
    efficient.capabilities = vec![efficient_offer];
    efficient
        .resources
        .iter_mut()
        .find(|pool| pool.class_id == efficient.capabilities[0].resource_requirements[0].class_id)
        .expect("efficient resource pool exists")
        .capacity_units = 4;
    let mut capable = source;
    capable.host_id = HostId::from("host-b-capable");
    capable.capabilities = vec![capable_offer];
    capable
        .resources
        .iter_mut()
        .find(|pool| pool.class_id == capable.capabilities[0].resource_requirements[0].class_id)
        .expect("capable resource pool exists")
        .capacity_units = 4;
    [efficient, capable]
}
