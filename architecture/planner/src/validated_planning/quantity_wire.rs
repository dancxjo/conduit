//! Optional Quantity representation compatibility, before Cord/Plan sealing.
use crate::PlannerError;
use alloc::collections::BTreeMap;
use conduit_core::{GearId, PlannedGear, QuantityValueProfile, QuantityWireContract};
use conduit_plot::CheckedConnection;

pub(super) fn contracts(
    gears: &[PlannedGear],
) -> Result<BTreeMap<GearId, QuantityWireContract>, PlannerError> {
    let mut result = BTreeMap::new();
    if gears
        .iter()
        .all(|gear| gear.realization_properties.is_empty())
    {
        return Ok(result);
    }
    let profile = QuantityWireContract::property_kind();
    for gear in gears {
        let Some(property) = gear
            .realization_properties
            .iter()
            .find(|p| p.profile() == &profile)
        else {
            continue;
        };
        let invalid = || PlannerError::InvalidRealizationProperties(gear.capability_id.clone());
        let contract = QuantityWireContract::from_property(property).map_err(|_| invalid())?;
        if contract
            .input_ports()
            .iter()
            .any(|(id, _)| !gear.inputs.iter().any(|p| &p.port_id == id))
            || contract
                .output_ports()
                .iter()
                .any(|(id, _)| !gear.outputs.iter().any(|p| &p.port_id == id))
        {
            return Err(invalid());
        }
        result.insert(gear.gear_id.clone(), contract);
    }
    Ok(result)
}

pub(super) fn validate_connection(
    connection: &CheckedConnection,
    contracts: &BTreeMap<GearId, QuantityWireContract>,
) -> Result<(), PlannerError> {
    if connection.track != conduit_core::ConnectionTrack::Payload {
        return Ok(());
    }
    let producer = contracts
        .get(&connection.source_gear_id)
        .map_or(QuantityValueProfile::Decimal, |c| {
            c.output(&connection.source_port_id)
        });
    let consumer = contracts
        .get(&connection.sink_gear_id)
        .map_or(QuantityValueProfile::Decimal, |c| {
            c.input(&connection.sink_port_id)
        });
    if consumer.accepts(producer) {
        Ok(())
    } else {
        Err(PlannerError::QuantityWireProfileMismatch(
            alloc::boxed::Box::new((
                connection.source_gear_id.clone(),
                connection.source_port_id.clone(),
                connection.sink_gear_id.clone(),
                connection.sink_port_id.clone(),
            )),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use conduit_core::{
        kind_id, ConnectionTrack, PortTemporal,
        QuantityValueProfile::{Decimal, Exact},
    };

    #[test]
    fn exact_wire_edges_require_the_selected_consumer_port_to_admit_them() {
        let mut connection = CheckedConnection {
            source_gear_id: "source".into(),
            source_port_id: "result".into(),
            sink_gear_id: "sink".into(),
            sink_port_id: "input".into(),
            value_kind: kind_id(conduit_core::QUANTITY_INFO_ID),
            track: ConnectionTrack::Payload,
            temporal: PortTemporal::Value,
        };
        let mut contracts = BTreeMap::new();
        assert!(validate_connection(&connection, &contracts).is_ok());
        contracts.insert(
            "source".into(),
            QuantityWireContract::new(vec![], vec![("result".into(), Exact)]).unwrap(),
        );
        let why = validate_connection(&connection, &contracts).unwrap_err();
        assert!(matches!(why, PlannerError::QuantityWireProfileMismatch(_)));
        assert!(why.to_string().contains("quantity/wire-profile-mismatch@1"));
        contracts.insert(
            "sink".into(),
            QuantityWireContract::new(
                vec![("other".into(), Exact), ("input".into(), Decimal)],
                vec![],
            )
            .unwrap(),
        );
        assert!(validate_connection(&connection, &contracts).is_err());
        contracts.insert(
            "sink".into(),
            QuantityWireContract::new(vec![("input".into(), Exact)], vec![]).unwrap(),
        );
        assert!(validate_connection(&connection, &contracts).is_ok());
        connection.track = ConnectionTrack::NormalClose;
        contracts.remove(&connection.sink_gear_id);
        assert!(validate_connection(&connection, &contracts).is_ok());
        connection.track = ConnectionTrack::Payload;
        connection.source_port_id = "native".into();
        assert!(validate_connection(&connection, &contracts).is_ok());
    }

    #[test]
    fn ordinary_planning_rejects_contract_ports_absent_from_the_selected_back() {
        let plot = crate::tests::plot();
        let mut host = crate::tests::host();
        let offer = &mut host.capabilities[0];
        let id = offer.capability_id.clone();
        offer.realization_properties =
            vec![
                QuantityWireContract::new(vec![], vec![("absent".into(), Exact)])
                    .unwrap()
                    .to_property()
                    .unwrap(),
            ];
        let hosts = [host];
        let placements = crate::default_placements(&plot, &hosts).unwrap();
        let why = crate::plan(
            &plot,
            &hosts,
            &placements,
            &[conduit_core::BaseImplementationId::from(
                conduit_core::LOCAL_BASE_IMPLEMENTATION_ID,
            )],
        )
        .unwrap_err();
        assert_eq!(why, PlannerError::InvalidRealizationProperties(id));
    }
}
