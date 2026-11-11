//! Selected realization facts accompany semantic graph and observed traffic.
use conduit_core::PlanFragment;
use conduit_face::{FaceProperty, FacePropertyValue as Value};
use patchbay_application::PatchbayApplicationPort;

pub(super) fn append(
    fragment: &PlanFragment,
    patchbay: &PatchbayApplicationPort,
    properties: &mut Vec<FaceProperty>,
) -> Result<(), String> {
    let graph = patchbay.graph();
    if graph.expanded_plot_id != fragment.expanded_plot_id
        || graph.checked_plot_id != fragment.checked_plot_id
    {
        return Err("replay realization differs from its semantic graph".into());
    }
    let mut fact = |subject: &str, name: &str, value| {
        properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        })
    };
    for placement in &fragment.placements {
        let gear = graph
            .gears
            .iter()
            .find(|gear| gear.gear_id == placement.gear_id)
            .ok_or("replay realization Gear is absent")?;
        let subject = format!("gear/{}", gear.identity);
        for (name, value) in [
            ("placement-id", placement.placement_id.as_str()),
            ("host-id", placement.host_id.as_str()),
            ("boot-id", placement.boot_id.as_str()),
            ("implementation-id", placement.implementation_id.as_str()),
            ("artifact-id", placement.artifact_id.as_str()),
            (
                "execution-profile-id",
                placement.execution_profile_id.as_str(),
            ),
        ] {
            fact(&subject, name, Value::Identity(value.into()));
        }
        fact(
            &subject,
            "maximum-queue-items",
            Value::Count(placement.limits.max_queue_items as u64),
        );
        fact(
            &subject,
            "maximum-queue-bytes",
            Value::Count(placement.limits.max_queue_bytes as u64),
        );
        for configuration in &placement.configuration {
            if configuration.key.starts_with("maximum-") {
                if let conduit_core::ConfigurationValue::U64(value) = configuration.value {
                    fact(&subject, &configuration.key, Value::Count(value));
                }
            }
        }
        if let Some(maximum) = placement
            .host_calls
            .iter()
            .map(|call| call.maximum_in_flight)
            .max()
        {
            fact(&subject, "maximum-in-flight", Value::Count(maximum as u64));
        }
    }
    for connection in &fragment.connections {
        let source_id = &fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == connection.source_placement_id)
            .ok_or("replay source placement is absent")?
            .gear_id;
        let sink_id = &fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == connection.sink_placement_id)
            .ok_or("replay sink placement is absent")?
            .gear_id;
        let source = graph
            .gears
            .iter()
            .find(|gear| &gear.gear_id == source_id)
            .and_then(|gear| {
                gear.outputs
                    .iter()
                    .find(|port| port.descriptor.port_id == connection.source_port_id)
            })
            .ok_or("replay source Port is absent")?;
        let sink = graph
            .gears
            .iter()
            .find(|gear| &gear.gear_id == sink_id)
            .and_then(|gear| {
                gear.inputs
                    .iter()
                    .find(|port| port.descriptor.port_id == connection.sink_port_id)
            })
            .ok_or("replay sink Port is absent")?;
        let cord = graph
            .cords
            .iter()
            .find(|cord| cord.source_port == source.identity && cord.sink_port == sink.identity)
            .ok_or("replay realization Cord is absent")?;
        let subject = format!("cord/{}", cord.identity);
        fact(
            &subject,
            "admitted-capacity",
            Value::Text(format!(
                "items={} bytes={}",
                connection.item_capacity, connection.byte_capacity
            )),
        );
        let policy =
            serde_json::to_value(connection.pressure_policy).map_err(|error| error.to_string())?;
        let policy = policy
            .as_str()
            .ok_or("replay pressure policy is not an identity")?;
        fact(
            &subject,
            "pressure",
            Value::Text(format!("policy: {policy}")),
        );
        fact(
            &subject,
            "maximum-queue-items",
            Value::Count(connection.item_capacity as u64),
        );
        fact(
            &subject,
            "maximum-queue-bytes",
            Value::Count(connection.byte_capacity as u64),
        );
    }
    Ok(())
}
