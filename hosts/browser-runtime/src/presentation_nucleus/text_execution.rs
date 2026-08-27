use super::{
    debug_error,
    offers::{text_advertisement, text_fixture_catalog, text_fixture_startup_catalog},
    operation::NucleusOperation,
    FIXTURE_TEXT_KIND, PORTS,
};
use conduit_core::ConnectionBase;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, ValueStorage,
};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use std::collections::BTreeMap;

const TEXT_FORM: &str = r#"form browser-text-nucleus {
 source: browser-fixture/text-source
 upper: text/upper
 present: presentation/text
 source > upper > present
}"#;

type TextScheduler = FixedScheduler<
    OperationDriver<NucleusOperation, PORTS>,
    FixedValueStore<6, { conduit_text::MAX_TEXT_BYTES as usize }>,
    FixedSignLog<32>,
    3,
    2,
    PORTS,
    2,
    { 3 * PORTS },
    2,
    4,
    3,
>;

pub(super) fn execute_text_form() -> Result<(String, conduit_core::PlanId), String> {
    let catalog = text_fixture_catalog()?;
    let startup = text_fixture_startup_catalog()?;
    let form = conduit_form::parse_with_startup(TEXT_FORM, &startup, &catalog)
        .map_err(|error| format!("parse browser text Form: {error:?}"))?;
    let advertisement = text_advertisement();
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts)
        .map_err(|error| format!("place browser text Form: {error:?}"))?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_text::MAX_TEXT_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| format!("plan browser text Form: {error:?}"))?;
    let fragment = plan
        .fragments
        .first()
        .ok_or_else(|| "browser text Plan has no fragment".to_string())?;
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower browser text Plan: {error:?}"))?;
    if fragment.placements.len() != 3
        || fragment.connections.len() != 2
        || lowered.nodes.len() != 3
        || lowered.cords.len() != 2
    {
        return Err("browser text Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser text node table has the wrong size".to_string())?;
    let cords = [lowered.cords[0].spec, lowered.cords[1].spec];
    let mut routes = FixedRoutes::<{ 3 * PORTS }, 2>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(debug_error)?;
    }
    routes.seal().map_err(debug_error)?;
    let mut bindings = FixedHostOperationBindings::<4>::new(1);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let mut values = FixedValueStore::<6, { conduit_text::MAX_TEXT_BYTES as usize }>::new(
        conduit_text::MAX_TEXT_BYTES * 6,
    )
    .map_err(debug_error)?;
    let source = values.store("Straße".as_bytes()).map_err(debug_error)?;
    let mut drivers = Vec::with_capacity(3);
    for (index, placement) in fragment.placements.iter().enumerate() {
        let operation = match placement.kind_id.as_str() {
            FIXTURE_TEXT_KIND => NucleusOperation::Source {
                value: source,
                emitted: false,
            },
            conduit_text::TEXT_UPPER_KIND => NucleusOperation::Transform {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                emitted: false,
            },
            conduit_std_catalog::TEXT_PRESENTATION_KIND => NucleusOperation::Sink {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                complete: false,
            },
            _ => return Err("browser text Plan selected an unsupported Kind".into()),
        };
        if index != drivers.len() {
            return Err("browser text placements are not in lowered node order".into());
        }
        drivers.push(OperationDriver::new(operation).map_err(debug_error)?);
    }
    let drivers = drivers
        .try_into()
        .map_err(|_| "browser text driver table is incomplete".to_string())?;
    let signs = FixedSignLog::<32>::new(
        lowered
            .sign_bytes
            .max((32 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(debug_error)?;
    let mut scheduler = TextScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(debug_error)?;
    let mut manifested = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                .to_vec();
            let placement = &fragment.placements[usize::from(request.node.0)];
            let outcome = if placement.kind_id.as_str() == conduit_text::TEXT_UPPER_KIND {
                let output = uppercase_utf8(&input)?;
                let value = scheduler.store_host_value(&output).map_err(debug_error)?;
                Some(
                    conduit_kernel::BoundedValueRef::new(
                        value,
                        placement.host_operations[0].maximum_output_bytes,
                    )
                    .map_err(|_| "uppercase output exceeded its planned bound")?,
                )
            } else if placement.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND {
                if manifested.replace(input).is_some() {
                    return Err("browser text manifested more than once".into());
                }
                None
            } else {
                return Err("browser text host request has an unsupported Kind".into());
            };
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: outcome,
                        failure: None,
                    },
                )
                .map_err(debug_error)?;
            continue;
        }
        match scheduler.step().map_err(debug_error)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => return Err("browser text kernel became idle".into()),
            SchedulerStatus::Cancelled => return Err("browser text kernel was cancelled".into()),
        }
    }
    let encoded = manifested.ok_or_else(|| "browser text produced no manifestation".to_string())?;
    let text = String::from_utf8(encoded)
        .map_err(|_| "browser text manifestation is not UTF-8".to_string())?;
    Ok((text, plan.plan_id))
}

pub(crate) fn uppercase_utf8(input: &[u8]) -> Result<Vec<u8>, String> {
    let text = core::str::from_utf8(input)
        .map_err(|_| "browser text/upper input is not valid UTF-8".to_string())?;
    let mut output = Vec::with_capacity(conduit_text::MAX_TEXT_BYTES as usize);
    for character in text.chars().flat_map(char::to_uppercase) {
        let mut encoded = [0_u8; 4];
        let bytes = character.encode_utf8(&mut encoded).as_bytes();
        if output.len() + bytes.len() > conduit_text::MAX_TEXT_BYTES as usize {
            return Err("browser text/upper output exceeds its admitted bound".into());
        }
        output.extend_from_slice(bytes);
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::uppercase_utf8;

    #[test]
    fn uppercase_is_utf8_exact_and_rejects_invalid_input() {
        assert_eq!(uppercase_utf8("Straße".as_bytes()).unwrap(), b"STRASSE");
        assert_eq!(
            uppercase_utf8(&[0xff]),
            Err("browser text/upper input is not valid UTF-8".into())
        );
    }
}
