//! One ordinary structured education Form executed by the browser kernel.

use super::{
    debug_error, NucleusOperation, BROWSER_PRESENTATION_ARTIFACT, BROWSER_PRESENTATION_PROFILE,
    PORTS,
};
use conduit_core::{
    bind_active_play, bind_presentation, bind_sign, BaseImplementationId, ConfigurationValue,
    Observation, ObservationKind, ValuePayload, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ProfileCatalog,
    StartupCatalog,
};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, HostOperationDisposition,
    HostOperationOutcome, HostedValueStore, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use conduit_planner::{plan_expanded_canonical_with_options, PlanningOptions};
use std::collections::BTreeMap;

const SOURCE: &str = "form browser-education-feedback {\n value: structured-info/literal(value = {outcome: passed(true), prompt_id: \"question/3\", score: 88%})\n show: presentation/structured-info\n value > show\n}\n";
const NODES: usize = 2;
const CORDS: usize = 1;
const ROUTES: usize = NODES * FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

type StructuredScheduler = FixedScheduler<
    OperationDriver<NucleusOperation, PORTS>,
    HostedValueStore,
    FixedSignLog<32>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    4,
    NODES,
>;

pub(super) fn execute() -> Result<(Observation, conduit_core::PlanId), String> {
    let value_type = conduit_std_catalog::education_feedback_type();
    let default = conduit_std_catalog::education_feedback_example();
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_structured_value_catalogs(
        conduit_std_catalog::EDUCATION_FEEDBACK_TYPE,
        &value_type,
        &default,
        &mut startup,
        &mut profile,
    )?;
    let syntax = parse_syntax_document(SOURCE);
    if !syntax.diagnostics.is_empty() {
        return Err("browser education Form has syntax diagnostics".into());
    }
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check browser education Form: {error:?}"))?;
    let expanded = expand_canonical_form(&checked, "browser-education-feedback", &profile)
        .map_err(|error| format!("expand browser education Form: {error:?}"))?;

    let mut literal = conduit_std_catalog::structured_literal_std_offer(
        conduit_std_catalog::EDUCATION_FEEDBACK_TYPE,
        &value_type,
    );
    let mut presenter = conduit_std_catalog::structured_presentation_std_offer(
        conduit_std_catalog::EDUCATION_FEEDBACK_TYPE,
        &value_type,
    );
    for offer in [&mut literal, &mut presenter] {
        offer.implementation.execution_profile_id = BROWSER_PRESENTATION_PROFILE.into();
        offer.implementation.artifact_id = BROWSER_PRESENTATION_ARTIFACT.into();
        offer.capability_id = format!("browser-{}@1", offer.kind_id.as_str())
            .as_str()
            .into();
        offer.implementation.implementation_id =
            format!("browser/kernel-{}@1", offer.kind_id.as_str())
                .as_str()
                .into();
    }
    let advertisement = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "browser-structured-presentation-host".into(),
        boot_id: "browser-structured-presentation-boot".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "browser/structured-presentation@1".into(),
        resources: vec![conduit_core::resource_offer(
            "browser-structured-presentation-slot",
            conduit_core::PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![literal, presenter],
    };
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .map_err(|error| format!("place browser education Form: {error:?}"))?;
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|error| format!("plan browser education Form: {error:?}"))?;
    let fragment = plan
        .fragments
        .first()
        .ok_or("browser education Plan has no fragment")?;
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower browser education Plan: {error:?}"))?;
    if fragment.placements.len() != NODES || lowered.cords.len() != CORDS {
        return Err("browser education Plan has an unexpected finite shape".into());
    }

    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "browser education node table")?;
    let cords = [lowered.cords[0].spec];
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
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
    let mut bindings = FixedHostOperationBindings::<4>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug_error)?;
    }
    bindings.seal().map_err(debug_error)?;
    let mut values = HostedValueStore::new(
        4,
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        (4 * MAXIMUM_STRUCTURED_CANONICAL_BYTES) as u32,
    )
    .map_err(debug_error)?;
    let mut drivers = Vec::with_capacity(NODES);
    for placement in &fragment.placements {
        let operation = match placement.kind_id.as_str() {
            conduit_std_catalog::STRUCTURED_LITERAL_KIND => {
                let encoded = placement
                    .configuration
                    .iter()
                    .find_map(|entry| match (&*entry.key, &entry.value) {
                        ("value", ConfigurationValue::Structured(value)) => {
                            Some(value.canonical_value())
                        }
                        _ => None,
                    })
                    .ok_or("browser structured literal has no exact value")?;
                NucleusOperation::Source {
                    value: values.store(encoded).map_err(debug_error)?,
                    emitted: false,
                }
            }
            conduit_std_catalog::STRUCTURED_PRESENTATION_KIND => NucleusOperation::Sink {
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
                pending: false,
                complete: false,
            },
            _ => return Err("browser education Plan selected an unsupported Kind".into()),
        };
        drivers.push(OperationDriver::new(operation).map_err(debug_error)?);
    }
    let drivers: [_; NODES] = drivers
        .try_into()
        .map_err(|_| "browser education driver table")?;
    let signs = FixedSignLog::<32>::new(
        lowered
            .sign_bytes
            .max((32 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(debug_error)?;
    let mut scheduler = StructuredScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(debug_error)?;
    let mut captured = Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    let mut capture_identity = None;
    let value_capacity = scheduler.values().allocation_capacities();
    loop {
        if let Some(request) = scheduler.next_host_request() {
            captured.extend_from_slice(
                scheduler
                    .host_value(request.input.value)
                    .map_err(debug_error)?,
            );
            capture_identity = Some(request);
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
            SchedulerStatus::Idle => return Err("browser education kernel became idle".into()),
            SchedulerStatus::Cancelled => {
                return Err("browser education kernel was cancelled".into())
            }
        }
    }
    if scheduler.values().allocation_capacities() != value_capacity {
        return Err("browser education Play changed admitted value capacity".into());
    }
    let request = capture_identity.ok_or("browser education value was not presented")?;
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let placement = &fragment.placements[usize::from(request.node.0)];
    let connection = fragment
        .connections
        .first()
        .ok_or("browser education Cord is missing")?;
    let presentation = bind_presentation(&active.active_play_id, &placement.placement_id, 0);
    let sign = bind_sign(
        &fragment.host_id,
        &fragment.boot_id,
        Some(&active.active_play_id),
        0,
    );
    let observation = Observation {
        sign_id: sign.sign_id,
        active_play_id: Some(active.active_play_id),
        presentation_id: Some(presentation.presentation_id),
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: Some(placement.placement_id.clone()),
        connection_id: Some(connection.connection_id.clone()),
        kind: ObservationKind::ValuePresented {
            value: ValuePayload {
                value_kind: value_type
                    .profile()
                    .map_err(debug_error)?
                    .value_kind()
                    .clone(),
                encoded: captured,
            },
        },
    };
    Ok((observation, fragment.plan_id.clone()))
}
