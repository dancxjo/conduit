//! Host, capability, and optional Device context in the topology projection.

use conduit_observatory::ObservatoryReport;

use crate::topology::TopologyViewError;

pub(crate) fn current_device_for_placement<'a>(
    report: &'a ObservatoryReport,
    placement: &conduit_observatory::PlacementRow,
) -> Option<&'a conduit_core::DeviceAssociation> {
    current_device_for_capability(
        report,
        &placement.host_id,
        &placement.boot_id,
        &placement.capability_id,
    )
    .filter(|device| device.offer_generation == placement.offer_generation)
}

pub fn current_device_for_capability<'a>(
    report: &'a ObservatoryReport,
    host_id: &conduit_core::HostId,
    boot_id: &conduit_core::BootId,
    capability_id: &conduit_core::CapabilityId,
) -> Option<&'a conduit_core::DeviceAssociation> {
    report.devices.iter().find_map(|row| {
        let association = &row.association;
        (matches!(
            association.disposition,
            conduit_core::DeviceTruthDisposition::Current
        ) && &association.host_id == host_id
            && &association.boot_id == boot_id
            && association.capability_ids.contains(capability_id))
        .then_some(association)
    })
}

pub(crate) fn render_hosts(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    let mut hosts = report.hosts.iter().collect::<Vec<_>>();
    hosts.sort_by(|left, right| {
        (&left.host_id, &left.boot_id).cmp(&(&right.host_id, &right.boot_id))
    });
    super::topology::push_line(lines, format!("HOSTS {}", hosts.len()))?;
    for host in hosts {
        super::topology::push_line(
            lines,
            format!(
                "  HOST {} / BOOT {} state={:?} profile={} generation={}",
                host.host_id.as_str(),
                host.boot_id.as_str(),
                host.state,
                host.profile.as_str(),
                host.offer_generation.0
            ),
        )?;
        let mut capabilities = report
            .capabilities
            .iter()
            .filter(|row| row.host_id == host.host_id && row.boot_id == host.boot_id)
            .collect::<Vec<_>>();
        capabilities.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        for capability in capabilities {
            super::topology::push_line(lines, format!(
                "    OFFER {} kind={} contract={} implementation={} support={:?} availability={:?} freshness={:?}",
                capability.capability_id.as_str(), capability.kind_id.as_str(),
                capability.kind_contract_revision.as_str(), capability.implementation_id.as_str(),
                capability.support, capability.availability, capability.freshness
            ))?;
            if let Some(device) = current_device_for_capability(
                report,
                &host.host_id,
                &host.boot_id,
                &capability.capability_id,
            ) {
                super::topology::push_line(
                    lines,
                    format!(
                        "      DEVICE {} identity={:?} provider={} resources={:?}",
                        device.device_id.as_str(),
                        device.identity_evidence.strength,
                        device.identity_evidence.provider.as_str(),
                        device.resources
                    ),
                )?;
            } else {
                super::topology::push_line(lines, "      DEVICE not identified".into())?;
            }
        }
        let mut planners = host.planner_capabilities.iter().collect::<Vec<_>>();
        planners.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
        for planner in planners {
            super::topology::push_line(
                lines,
                format!(
                    "    PLANNER {} hosts={} operations={} connections={}",
                    planner.profile_id.as_str(),
                    planner.limits.maximum_host_advertisements,
                    planner.limits.maximum_gears,
                    planner.limits.maximum_connections
                ),
            )?;
        }
        let mut resources = host.resources.iter().collect::<Vec<_>>();
        resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
        for resource in resources {
            super::topology::push_line(
                lines,
                format!(
                    "    RESOURCE {} class={} capacity={}",
                    resource.pool_id.as_str(),
                    resource.class_id.as_str(),
                    resource.capacity_units
                ),
            )?;
        }
    }
    for row in report.devices.iter().filter(|row| {
        matches!(
            row.association.disposition,
            conduit_core::DeviceTruthDisposition::HistoricalLost { .. }
        )
    }) {
        super::topology::push_line(
            lines,
            format!(
                "  HISTORICAL DEVICE {} host={} boot={} generation={} disposition={:?}",
                row.association.device_id.as_str(),
                row.association.host_id.as_str(),
                row.association.boot_id.as_str(),
                row.association.offer_generation.0,
                row.association.disposition
            ),
        )?;
    }
    Ok(())
}
