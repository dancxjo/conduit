use super::{
    debug_error,
    offers::{text_advertisement, text_fixture_catalog},
    operation::NucleusOperation,
    FIXTURE_TEXT_KIND, PORTS,
};
use conduit_core::ConnectionBase;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, ValueStorage,
};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use conduit_runtime::lowering::lower_plan_fragment;
use std::collections::BTreeMap;

const TEXT_FORM: &str = r#"form 0

browser-text-nucleus {
 source: browser.fixture/text-source
 present: presentation/text
 source.text -> present.text
}"#;

type TextScheduler = FixedScheduler<
    OperationDriver<NucleusOperation, PORTS>,
    FixedValueStore<4, { conduit_std_catalog::MAX_TEXT_BYTES as usize }>,
    FixedSignLog<32>,
    2,
    1,
    PORTS,
    1,
    { 2 * PORTS },
    1,
    4,
    2,
>;

pub(super) fn execute_text_form() -> Result<(String, conduit_core::PlanId), String> {
    let catalog = text_fixture_catalog()?;
    let form = conduit_form::parse(TEXT_FORM, &catalog)
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
            connection_byte_capacity: conduit_std_catalog::MAX_TEXT_BYTES,
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
    if fragment.placements.len() != 2
        || fragment.connections.len() != 1
        || lowered.nodes.len() != 2
        || lowered.cords.len() != 1
    {
        return Err("browser text Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser text node table has the wrong size".to_string())?;
    let cords = [lowered.cords[0].spec];
    let mut routes = FixedRoutes::<{ 2 * PORTS }, 1>::new(PORTS as u16);
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
    let mut bindings = FixedHostOperationBindings::<4>::new(2);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let mut values = FixedValueStore::<4, { conduit_std_catalog::MAX_TEXT_BYTES as usize }>::new(
        conduit_std_catalog::MAX_TEXT_BYTES * 4,
    )
    .map_err(debug_error)?;
    let source = values.store(b"Gear Face").map_err(debug_error)?;
    let mut drivers = [None, None];
    for (index, placement) in fragment.placements.iter().enumerate() {
        let operation = if placement.kind_id.as_str() == FIXTURE_TEXT_KIND {
            NucleusOperation::Source {
                value: source,
                emitted: false,
            }
        } else if placement.kind_id.as_str() == conduit_std_catalog::TEXT_PRESENTATION_KIND {
            NucleusOperation::Sink {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                complete: false,
            }
        } else {
            return Err("browser text Plan selected an unsupported Kind".into());
        };
        drivers[index] = Some(OperationDriver::new(operation).map_err(debug_error)?);
    }
    let [Some(first), Some(second)] = drivers else {
        return Err("browser text driver table is incomplete".into());
    };
    let signs = FixedSignLog::<32>::new(
        lowered
            .sign_bytes
            .max((32 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(debug_error)?;
    let mut scheduler = TextScheduler::new_with_host_operations(
        nodes,
        cords,
        routes,
        bindings,
        [first, second],
        values,
        signs,
    )
    .map_err(debug_error)?;
    let mut manifested = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                .to_vec();
            if manifested.replace(input).is_some() {
                return Err("browser text manifested more than once".into());
            }
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: None,
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
