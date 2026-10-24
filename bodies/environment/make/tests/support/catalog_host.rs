//! Shared CPU Host/Body-admission fixture for installed catalogue proofs.
use conduit_core as c;
use conduit_environment_make::RecordingCatalogSeedSource;
use conduit_std_host as host;
use std::collections::BTreeMap;
pub(crate) fn planned(write: bool, boot: &str) -> (host::StdHost, c::Plan, c::StructuredInfoType) {
    let source = if write {
        RecordingCatalogSeedSource::compile()
    } else {
        RecordingCatalogSeedSource::compile_read_only()
    }
    .unwrap();
    let advertisement = configured(&source, boot);
    let (host, plan) = plan_source(&source, advertisement, &[], 1024);
    (host, plan, source.value_type)
}
pub(crate) fn configured(source: &RecordingCatalogSeedSource, boot: &str) -> c::HostAdvertisement {
    let gear = source
        .plot
        .expanded
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == c::STATE_VALUE_KIND)
        .unwrap();
    let c::ConfigurationValue::Structured(initial) = &gear
        .configuration
        .iter()
        .find(|entry| entry.key == "initial")
        .unwrap()
        .value
    else {
        panic!("initial")
    };
    let initial = c::StructuredInfoValue::from_canonical_bytes(initial.canonical_value()).unwrap();
    let mut advertisement = host::StdHost::new_with_config(host::StdHostConfig {
        host_id: "fixture/seed-host".into(),
        boot_id: boot.into(),
        offer_generation: c::OfferGeneration(1),
    })
    .advertisement()
    .clone();
    advertisement
        .capabilities
        .retain(|offer| offer.kind_id.as_str() != c::STATE_VALUE_KIND);
    advertisement.capabilities.push(
        conduit_std_offers::state_value_durable_std_offer(
            gear.startup_parameters[0].value_type.as_str(),
            &source.value_type,
            &initial,
        )
        .unwrap(),
    );
    advertisement.resources.push(c::ResourceOffer {
        pool_id: "fixture/seed-state".into(),
        class_id: conduit_std_offers::STATE_VALUE_DURABLE_RESOURCE_CLASS.into(),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    advertisement
}
pub(crate) fn plan_source(
    source: &RecordingCatalogSeedSource,
    mut advertisement: c::HostAdvertisement,
    grants: &[c::AuthorityGrant],
    bytes: u32,
) -> (host::StdHost, c::Plan) {
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    advertisement
        .resources
        .sort_by(|a, b| a.pool_id.cmp(&b.pool_id));
    let hosts = [advertisement.clone()];
    let choices =
        conduit_planner::default_expanded_placements(&source.plot.expanded, &hosts).unwrap();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (c::PortDirection::Input, &source.plot.input_bindings),
        (c::PortDirection::Output, &source.plot.output_bindings),
    ] {
        for binding in bindings {
            boundaries.insert(
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: binding.front_port_id.clone(),
                    track: binding.track,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            );
        }
    }
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &source.plot,
        &hosts,
        &choices,
        &["conduit.base/local@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: bytes,
            authority_grants: grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (
        host::StdHost::from_advertisement(advertisement).unwrap(),
        plan,
    )
}
pub(crate) struct WriteCurrent {
    pub(crate) expected: Vec<u8>,
    pub(crate) control: host::RunControl,
}
impl host::ExternalForeOutputAdapter for WriteCurrent {
    fn deliver(&mut self, value: host::ExternalForeDelivery) -> Result<(), String> {
        if value.bytes.as_slice() == self.expected {
            self.control
                .request_stop(host::RunControlRequestId::new("seed-written").unwrap())
                .map_err(|error| format!("stop: {error:?}"))?;
        }
        Ok(())
    }
}
pub(crate) struct NoWait;
impl host::TimerAdapter for NoWait {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("seed recovery needs no timer wait")
    }
}
