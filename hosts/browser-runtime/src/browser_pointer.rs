//! A real browser pointer event entering one ordinary planned kernel Play.

use conduit_core::{
    bind_active_play, bind_sign, kind_id, port_id, resource_offer, ArtifactId, BootId,
    CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase, ExecutionProfileId,
    HostAdvertisement, HostId, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, ImplementationOffer, KindContractRevision, OfferGeneration, PortDescriptor,
    PortDirection, PortTemporal, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
    PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedValueStore, Operation,
    OperationAction, OperationInput, PortId, RequestId, ValueStorage,
};
use conduit_planner::{plan_expanded_canonical_with_options, PlanningOptions};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};
use conduit_std_catalog::{
    normalized_pointer_value, pointer_event_type, NormalizedPointerSample, POINTER_EVENT_TYPE,
    POINTER_SOURCE_KIND, STRUCTURED_PRESENTATION_KIND,
};
use std::collections::BTreeMap;

mod abi;
pub use abi::*;
mod receipt;
pub use receipt::BrowserPointerReceipt;
#[cfg(test)]
mod tests;

const SOURCE_OPERATION: &str = "browser.host/pointer-source@1";
const PROFILE: &str = "browser/pointer-source@1";
const ARTIFACT: &str = "conduit-browser-runtime/pointer-source@1";
const FORM_SOURCE: &str = "form browser-pointer {\n pointer: input/pointer-source\n show: presentation/structured-info\n pointer.pointer > show.input\n}\n";
const NODES: usize = 2;
const CORDS: usize = 1;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const ROUTES: usize = NODES * PORTS;
const VALUES: usize = 4;
const SIGNS: usize = 32;
const HOST_BINDINGS: usize = NODES * NODES;

type PointerScheduler = FixedScheduler<
    OperationDriver<PointerOperation, PORTS>,
    HostedValueStore,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    HOST_BINDINGS,
    NODES,
>;

enum PointerOperation {
    Source {
        empty: conduit_kernel::ValueRef,
        pending: bool,
        emitted: bool,
        maximum_output_bytes: u32,
    },
    Sink {
        pending: bool,
        complete: bool,
        maximum_input_bytes: u32,
    },
}

impl Operation for PointerOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Source {
                empty,
                pending,
                maximum_output_bytes,
                ..
            } if !*pending => {
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(*empty, 0)
                        .expect("empty browser pointer request is exactly bounded"),
                }
            }
            Self::Source { .. } | Self::Sink { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Source {
                    pending,
                    emitted,
                    maximum_output_bytes,
                    ..
                },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return fail(1);
                };
                if output.admitted_bytes != *maximum_output_bytes {
                    return fail(2);
                }
                *pending = false;
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (
                Self::Sink {
                    pending,
                    maximum_input_bytes,
                    ..
                },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if !*pending => {
                *pending = true;
                OperationAction::RequestHostOperation {
                    request: RequestId(0),
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, *maximum_input_bytes)
                        .expect("pointer presentation input is admitted"),
                }
            }
            (
                Self::Sink {
                    pending, complete, ..
                },
                OperationInput::HostOperationCompleted {
                    request: RequestId(0),
                    outcome,
                },
            ) if *pending
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = false;
                *complete = true;
                OperationAction::Await
            }
            (
                Self::Sink {
                    pending, complete, ..
                },
                OperationInput::Closed { port: PortId(0) },
            ) if !*pending && *complete => OperationAction::Complete,
            _ => fail(3),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Source { emitted: true, .. } => OperationAction::Complete,
            _ => OperationAction::Await,
        }
    }
}

fn fail(detail: u16) -> OperationAction {
    OperationAction::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

pub fn execute_browser_pointer(
    sample: NormalizedPointerSample,
) -> Result<BrowserPointerReceipt, String> {
    let value =
        normalized_pointer_value(sample).map_err(|error| format!("pointer value: {error:?}"))?;
    let canonical = value
        .canonical_bytes()
        .map_err(|error| format!("encode pointer value: {error:?}"))?;
    let (startup, profile) = catalogs(&value)?;
    let syntax = conduit_form::parse_syntax_document(FORM_SOURCE);
    if !syntax.diagnostics.is_empty() {
        return Err("browser pointer Form has syntax diagnostics".into());
    }
    let checked = conduit_form::check_syntax_document(&syntax, &startup)
        .map_err(|error| format!("check browser pointer Form: {error:?}"))?;
    let expanded = conduit_form::expand_canonical_form(&checked, "browser-pointer", &profile)
        .map_err(|error| format!("expand browser pointer Form: {error:?}"))?;
    let host = advertisement();
    let hosts = [host.clone()];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts)
        .map_err(|error| format!("place browser pointer Form: {error:?}"))?;
    let plan = plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
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
    .map_err(|error| format!("plan browser pointer Form: {error:?}"))?;
    let fragment = plan
        .fragments
        .first()
        .ok_or("browser pointer Plan has no fragment")?;
    let lowered = lower_plan_fragment(fragment)
        .map_err(|error| format!("lower browser pointer Plan: {error:?}"))?;
    let mut scheduler = scheduler(fragment, &lowered)?;
    let capacities = scheduler.values().allocation_capacities();
    let mut captured = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let placement = &fragment.placements[usize::from(request.node.0)];
            if placement.kind_id.as_str() == POINTER_SOURCE_KIND {
                let output = scheduler
                    .store_host_value(&canonical)
                    .map_err(|error| format!("store browser pointer value: {error:?}"))?;
                scheduler
                    .complete_host_operation(
                        request.node,
                        request.request,
                        HostOperationOutcome {
                            disposition: HostOperationDisposition::Completed,
                            output: Some(
                                BoundedValueRef::new(
                                    output,
                                    MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                                )
                                .map_err(|_| "browser pointer output bound")?,
                            ),
                            failure: None,
                        },
                    )
                    .map_err(|error| format!("complete browser pointer source: {error:?}"))?;
            } else if placement.kind_id.as_str() == STRUCTURED_PRESENTATION_KIND {
                let bytes = scheduler
                    .host_value(request.input.value)
                    .map_err(|error| format!("read browser pointer presentation: {error:?}"))?
                    .to_vec();
                if captured.replace(bytes).is_some() {
                    return Err("browser pointer was presented more than once".into());
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
                    .map_err(|error| format!("complete browser pointer presentation: {error:?}"))?;
            } else {
                return Err("browser pointer Plan selected an unsupported Kind".into());
            }
            continue;
        }
        match scheduler
            .step()
            .map_err(|error| format!("run browser pointer Play: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => return Err("browser pointer Play became idle".into()),
            SchedulerStatus::Cancelled => return Err("browser pointer Play was cancelled".into()),
        }
    }
    if captured.as_deref() != Some(canonical.as_slice()) {
        return Err("browser pointer canonical value changed in transit".into());
    }
    if scheduler.values().allocation_capacities() != capacities {
        return Err("browser pointer Play changed admitted value capacity".into());
    }
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let sign = bind_sign(
        &fragment.host_id,
        &fragment.boot_id,
        Some(&active.active_play_id),
        0,
    );
    Ok(BrowserPointerReceipt {
        plan_id: fragment.plan_id.as_str().to_string(),
        play_id: active.active_play_id.as_str().to_string(),
        sign_id: sign.sign_id.as_str().to_string(),
        source_placement_id: fragment.placements[0].placement_id.as_str().to_string(),
        presentation_placement_id: fragment.placements[1].placement_id.as_str().to_string(),
        schema: "input/pointer-event@1".to_string(),
        value_kind: pointer_event_type()
            .profile()
            .map_err(debug)?
            .value_kind()
            .as_str()
            .to_string(),
        canonical_bytes: canonical.len(),
        position_x: sample.position_x,
        position_y: sample.position_y,
        delta_x: sample.delta_x,
        delta_y: sample.delta_y,
        primary_pressed: sample.primary_pressed,
        coalesced: sample.coalesced,
        dropped: sample.dropped,
        queue_capacity: sample.queue_capacity,
        sequence: sample.sequence,
    })
}

fn catalogs(value: &StructuredInfoValue) -> Result<(StartupCatalog, ProfileCatalog), String> {
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    conduit_std_catalog::install_generalized_input_catalogs(&mut startup, &mut profile)?;
    startup
        .insert(KindSignature {
            kind: STRUCTURED_PRESENTATION_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    let presenter = conduit_std_catalog::structured_presentation_std_offer(
        POINTER_EVENT_TYPE,
        value.value_type(),
    );
    profile
        .insert(KindDefinition {
            kind_id: presenter.kind_id,
            kind_contract_revision: presenter.kind_contract_revision,
            inputs: presenter.inputs,
            outputs: presenter.outputs,
            configuration: Vec::new(),
        })
        .map_err(|error| error.to_string())?;
    Ok((startup, profile))
}

fn advertisement() -> HostAdvertisement {
    let value_type = pointer_event_type();
    let value_kind = value_type
        .profile()
        .expect("pointer profile")
        .value_kind()
        .clone();
    let source = CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("browser-pointer-source@1"),
        kind_id: kind_id(POINTER_SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::GENERALIZED_INPUT_REVISION,
        ),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from("browser/dom-pointer-source@1"),
            artifact_id: ArtifactId::from(ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("pointer"),
            value_kind: value_kind.clone(),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(SOURCE_OPERATION),
            target_kind: Some(kind_id(POINTER_SOURCE_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: 0,
            maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    };
    let mut presenter =
        conduit_std_catalog::structured_presentation_std_offer(POINTER_EVENT_TYPE, &value_type);
    presenter.capability_id = CapabilityId::from("browser-pointer-presentation@1");
    presenter.implementation.execution_profile_id = ExecutionProfileId::from(PROFILE);
    presenter.implementation.implementation_id =
        ImplementationId::from("browser/pointer-presentation@1");
    presenter.implementation.artifact_id = ArtifactId::from(ARTIFACT);
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("browser-pointer-host"),
        boot_id: BootId::from("browser-pointer-boot"),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(PROFILE),
        resources: vec![resource_offer(
            "browser-pointer-presentation-slot",
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![source, presenter],
    }
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_runtime::lowering::LoweredPlanFragment,
) -> Result<PointerScheduler, String> {
    if fragment.placements.len() != NODES || lowered.cords.len() != CORDS {
        return Err("browser pointer Plan has an unexpected finite shape".into());
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| "pointer nodes")?;
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
            .map_err(debug)?;
    }
    routes.seal().map_err(debug)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(debug)?;
    }
    bindings.seal().map_err(debug)?;
    let mut values = HostedValueStore::new(
        VALUES as u16,
        MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        (VALUES * MAXIMUM_STRUCTURED_CANONICAL_BYTES) as u32,
    )
    .map_err(debug)?;
    let empty = values.store(&[]).map_err(debug)?;
    let mut drivers = Vec::with_capacity(NODES);
    for placement in &fragment.placements {
        let operation = match placement.kind_id.as_str() {
            POINTER_SOURCE_KIND => PointerOperation::Source {
                empty,
                pending: false,
                emitted: false,
                maximum_output_bytes: placement.host_operations[0].maximum_output_bytes,
            },
            STRUCTURED_PRESENTATION_KIND => PointerOperation::Sink {
                pending: false,
                complete: false,
                maximum_input_bytes: placement.host_operations[0].maximum_input_bytes,
            },
            _ => return Err("unsupported browser pointer placement".into()),
        };
        drivers.push(OperationDriver::new(operation).map_err(debug)?);
    }
    let drivers = drivers.try_into().map_err(|_| "pointer drivers")?;
    let sign_bytes = SIGNS
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>())
        .and_then(|bytes| u32::try_from(bytes).ok())
        .ok_or("browser pointer Sign capacity overflow")?;
    let signs = FixedSignLog::<SIGNS>::new(sign_bytes).map_err(debug)?;
    PointerScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(debug)
}

fn debug(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
