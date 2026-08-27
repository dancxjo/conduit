use conduit_core::{ArtifactId, CapabilityId, HostId, ImplementationId, KindContractRevision};
use conduit_form::parse_with_startup;
use conduit_signal::{signal_profile_catalog, PULSE_KIND};
use conduit_signal_conformance::pico_local_advertisement;

#[allow(dead_code)]
pub fn standard_planning_fixture(
    host_id: impl Into<conduit_core::HostId>,
    boot_id: impl Into<conduit_core::BootId>,
) -> conduit_core::HostAdvertisement {
    conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: host_id.into(),
        boot_id: boot_id.into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: conduit_core::HostProfileId::from("planner-test/standard-fixture@1"),
        resources: vec![
            conduit_core::resource_offer(
                "planner-test/presentation",
                conduit_core::PRESENTATION_RESOURCE_CLASS,
                16,
            ),
            conduit_core::resource_offer(
                "planner-test/timer",
                conduit_core::TIMER_RESOURCE_CLASS,
                16,
            ),
        ],
        planner_capabilities: Vec::new(),
        capabilities: conduit_std_catalog::supported_nucleus_offers(),
    }
}

#[allow(dead_code)]
pub fn generate_text_form() -> conduit_form::CheckedForm {
    let mut startup = conduit_form::StartupCatalog::new();
    let mut profile = conduit_form::ProfileCatalog::new();
    conduit_ai::install_generate_text_catalog(&mut startup, &mut profile)
        .expect("catalog installs");
    conduit_form::parse_with_startup(
        "form answer {\n    generate: ai/generate-text\n}\n",
        &startup,
        &profile,
    )
    .expect("form checks")
}

#[allow(dead_code)]
pub fn generic_policy_facts() -> (
    conduit_form::CheckedForm,
    Vec<conduit_core::HostAdvertisement>,
    Vec<conduit_core::RealizationAdvertisement>,
) {
    let form = generate_text_form();
    let fixtures = conduit_ai::generate_text_base_fixtures();
    let advertisements = conduit_ai::generate_text_realization_advertisements(&fixtures);
    let mut hosts = fixtures
        .iter()
        .map(|fixture| fixture.advertisement.clone())
        .collect::<Vec<_>>();
    let large = hosts
        .iter_mut()
        .find(|host| host.host_id.as_str() == "ai-large-local")
        .expect("large local host exists");
    let cpu = large
        .resources
        .iter_mut()
        .find(|pool| pool.class_id.as_str() == conduit_ai::CPU_EXECUTION_RESOURCE)
        .expect("CPU pool exists");
    let compute = cpu.compute.as_mut().expect("CPU pool is typed compute");
    compute.service_guarantee = conduit_core::ComputeServiceGuarantee::Reserved;
    compute.topology_groups = vec![conduit_core::ComputeTopologyGroup {
        group_id: conduit_core::ComputeTopologyGroupId::from("cluster-performance"),
        lane_capacity: 2,
        numa_domain: Some(conduit_core::ComputeDomainId::from("numa-0")),
        cache_domain: Some(conduit_core::ComputeDomainId::from("cache-0")),
        performance_class: Some(conduit_core::ComputePerformanceClassId::from("performance")),
        nominal_clock_hz: Some(1_800_000_000),
    }];
    (form, hosts, advertisements)
}

#[allow(dead_code)]
pub fn resource_observations(
    hosts: &[conduit_core::HostAdvertisement],
) -> Vec<conduit_core::ResourceObservation> {
    hosts
        .iter()
        .flat_map(|host| {
            host.resources.iter().enumerate().map(move |(index, pool)| {
                conduit_core::ResourceObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    pool_id: pool.pool_id.clone(),
                    class_id: pool.class_id.clone(),
                    health: conduit_core::ResourceHealth::Ready,
                    unreserved_units: pool.capacity_units,
                    utilized_units: 0,
                    sign_id: conduit_core::SignId::from(format!(
                        "{}-observation-{index}",
                        host.host_id.as_str()
                    )),
                }
            })
        })
        .collect()
}

#[allow(dead_code)]
pub fn quantity(
    value: u64,
    unit: conduit_core::CharacteristicUnit,
) -> conduit_planner::PlannerFactValue {
    conduit_planner::PlannerFactValue::Quantity { value, unit }
}

#[allow(dead_code)]
pub fn pulse_gear() -> conduit_form::CheckedGear {
    parse_with_startup(
        "form realization {\n    pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .expect("pulse form checks")
    .gears
    .remove(0)
}

#[allow(dead_code)]
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
