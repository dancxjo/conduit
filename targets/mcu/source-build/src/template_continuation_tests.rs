//! Cross-Boot realization of the actual Source calibration's emitted Fore pins.
use super::*;
pub(super) fn assert_unchanged_fore(
    layout: &Layout,
    host: &c::HostAdvertisement,
    restarted: &c::HostAdvertisement,
) {
    // Both emitted representations describe the original checked bindings.
    let plain = layout
        .continuation
        .plain_resource_bindings
        .as_deref()
        .expect("this fixture has only plain resources");
    let decoded = conduit_body::PlainFragmentResources::new(plain)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(decoded.len(), layout.continuation.resources.len());
    let mut canonical = Vec::new();
    for (row, pin) in decoded.iter().zip(&layout.continuation.resources) {
        assert_eq!(
            (
                row.node,
                row.resource,
                row.placement,
                row.pool,
                row.class,
                row.units
            ),
            (
                pin.node,
                pin.resource,
                pin.placement.as_str(),
                pin.pool.as_str(),
                pin.class.as_str(),
                pin.units
            )
        );
        row.append_to(&mut canonical).unwrap();
    }
    assert_eq!(canonical, plain);
    assert!(layout
        .continuation
        .emit()
        .contains("INSTALLED_PLAIN_RESOURCE_BINDINGS:Option<&[u8]>=Some("));
    // Emitted named Fore pins preserve the canonical connection when only
    // Boot changes; those names are not runtime capabilities or epochs.
    let fresh_fore: Vec<_> = layout
        .continuation
        .fore
        .iter()
        .map(|pin| c::FreshAssignedFore {
            cord: pin.cord,
            endpoint: pin.endpoint,
            direction: if pin.input {
                c::AssignedForeDirection::Input
            } else {
                c::AssignedForeDirection::Output
            },
            local_node: pin.node,
            local_port: pin.port,
            front_port: &pin.front,
            placement: &pin.placement,
            gear_port: &pin.gear,
        })
        .collect();
    let mut fresh = vec![0; layout.projection_template.len()];
    c::AssignedLocalProjectionTemplate::check(
        &layout.projection_template,
        &layout.projection_template_digest,
        layout.maxima,
    )
    .unwrap()
    .instantiate(
        c::FreshAssignedHeader {
            plan: c::AssignedIdentity::from_text("template/fresh-plan"),
            fragment: c::AssignedIdentity::from_text("template/fresh-fragment"),
            host: c::AssignedIdentity::from_text(host.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(restarted.boot_id.as_str()),
        },
        &[],
        &fresh_fore,
        &mut fresh,
    )
    .unwrap();
    for pin in &fresh_fore {
        let original = c::AssignedPlanRecords::new(&layout.projection_template)
            .unwrap()
            .map(Result::unwrap)
            .find(|record| {
                record.tag == c::ASSIGNED_CORD
                    && u16::from_le_bytes(record.payload[..2].try_into().unwrap()) == pin.cord
            })
            .unwrap();
        let connection = c::assigned_local_fore_connection(
            pin.direction,
            pin.front_port,
            pin.placement,
            pin.gear_port,
        );
        assert_eq!(&original.payload[2..18], &connection.0);
    }
}
