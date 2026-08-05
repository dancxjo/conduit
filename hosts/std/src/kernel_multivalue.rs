//! Exact, typed planning profile for the std-host multi-value kernel gate.
//!
//! This is deliberately not the legacy `conduit.std` catalog: the filter has
//! concrete even-tick semantics and every port carries `value/tick@1`.

use super::TimerAdapter;
use conduit_core::{
    bind_active_play, bind_evidence, bind_presentation, kind_id, port_id,
    present_host_operation_requirement, resource_offer, resource_requirement,
    wait_host_operation_requirement, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConfigurationEntry, ConfigurationValue, ExecutionProfileId, HostAdvertisement, HostId,
    HostProfileId, KindContractRevision, Observation, ObservationKind, OfferGeneration,
    PlacementId, PlanFragment, PortDescriptor, PortDirection, PresentationId, TerminalDisposition,
    ValuePayload, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, TIMER_RESOURCE_CLASS,
};
use conduit_form::{ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, EvidenceSink, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedEvidenceLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId as KernelPortId,
    RequestId, ValueRef, ValueStorage,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};
use std::io::Write;
use std::time::Duration;

pub const TICK_KIND: &str = "time/tick";
pub const TEE_KIND: &str = "flow/tee";
pub const EVEN_FILTER_KIND: &str = "flow/filter-even";
pub const LATEST_KIND: &str = "state/latest";
pub const SHOW_KIND: &str = "presentation/show";
pub const TICK_VALUE_KIND: &str = "value/tick@1";

const IN_PORT: &str = "in";
const OUT_PORT: &str = "out";
const TICK_PORT: &str = "tick";
const LEFT_PORT: &str = "left";
const RIGHT_PORT: &str = "right";

const NODES: usize = 6;
const CORDS: usize = 5;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 20;
const ROUTE_SLOTS: usize = NODES * PORTS;
const ROUTE_TARGETS: usize = 5;
const HOST_BINDING_SLOTS: usize = NODES;
const PENDING_REQUESTS: usize = 3;

type MultiValueScheduler = FixedScheduler<
    OperationDriver<MultiValueOperation, PORTS>,
    HostedValueStore,
    HostedEvidenceLog,
    NODES,
    CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultiValueReceipt {
    pub placement_id: PlacementId,
    pub tick: u64,
}

#[derive(Debug, Clone)]
pub struct MultiValueRunReport {
    pub observations: Vec<Observation>,
    pub receipts: Vec<MultiValueReceipt>,
    pub active_play_id: conduit_core::ActivePlayId,
    pub decisions: u32,
    pub kernel_events: u16,
    pub value_allocation_capacity_before: (usize, usize),
    pub value_allocation_capacity_after: (usize, usize),
    pub presentation_ids: Vec<PresentationId>,
}

enum MultiValueOperation {
    Tick {
        values: Vec<ValueRef>,
        waits: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
    Tee {
        value: Option<ValueRef>,
        phase: u8,
    },
    FilterEven {
        admitted: Vec<ValueRef>,
    },
    Latest {
        held: Option<ValueRef>,
        released: Option<ValueRef>,
        retain_resumed: bool,
        closing: bool,
    },
    Show {
        expected: Vec<ValueRef>,
        next: usize,
        pending: Option<RequestId>,
    },
}

impl MultiValueOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    fn allocation_capacity(&self) -> usize {
        match self {
            Self::Tick { values, waits, .. } => values.capacity() + waits.capacity(),
            Self::FilterEven { admitted } => admitted.capacity(),
            Self::Show { expected, .. } => expected.capacity(),
            Self::Tee { .. } | Self::Latest { .. } => 0,
        }
    }
}

impl Operation for MultiValueOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Tick { waits, pending, .. } => {
                let Some(wait) = waits.first().copied() else {
                    return Self::fail(1);
                };
                let request = RequestId(0);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8).expect("sealed wait is eight bytes"),
                }
            }
            _ => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Tick {
                    values,
                    next,
                    pending,
                    ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                values.get(*next).copied().map_or_else(
                    || Self::fail(2),
                    |value| OperationAction::Emit {
                        port: KernelPortId(0),
                        value,
                    },
                )
            }
            (
                Self::Tee { value, phase },
                OperationInput::Value {
                    port: KernelPortId(0),
                    value: input,
                },
            ) => {
                *value = Some(input);
                *phase = 1;
                OperationAction::Emit {
                    port: KernelPortId(0),
                    value: input,
                }
            }
            (
                Self::Tee { .. },
                OperationInput::Closed {
                    port: KernelPortId(0),
                },
            ) => OperationAction::Complete,
            (
                Self::FilterEven { admitted },
                OperationInput::Value {
                    port: KernelPortId(0),
                    value,
                },
            ) => {
                if admitted.contains(&value) {
                    OperationAction::Emit {
                        port: KernelPortId(0),
                        value,
                    }
                } else {
                    OperationAction::Await
                }
            }
            (
                Self::FilterEven { .. },
                OperationInput::Closed {
                    port: KernelPortId(0),
                },
            ) => OperationAction::Complete,
            (
                Self::Latest {
                    held,
                    released,
                    retain_resumed,
                    ..
                },
                OperationInput::Value {
                    port: KernelPortId(0),
                    value,
                },
            ) => {
                *released = held.replace(value);
                *retain_resumed = true;
                OperationAction::Await
            }
            (
                Self::Latest {
                    held,
                    retain_resumed,
                    closing,
                    ..
                },
                OperationInput::Closed {
                    port: KernelPortId(0),
                },
            ) => {
                *retain_resumed = false;
                let Some(value) = held.take() else {
                    return OperationAction::Complete;
                };
                *closing = true;
                OperationAction::Emit {
                    port: KernelPortId(0),
                    value,
                }
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Value {
                    port: KernelPortId(0),
                    value,
                },
            ) if pending.is_none() && expected.get(*next) == Some(&value) => {
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(3);
                };
                let request = RequestId(0x8000_0000 | sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(value, 8).expect("sealed tick is eight bytes"),
                }
            }
            (
                Self::Show { next, pending, .. },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                *next += 1;
                OperationAction::Await
            }
            (
                Self::Show {
                    expected,
                    next,
                    pending,
                },
                OperationInput::Closed {
                    port: KernelPortId(0),
                },
            ) if pending.is_none() && *next == expected.len() => OperationAction::Complete,
            _ => Self::fail(4),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Tick {
                values,
                waits,
                next,
                pending,
            } => {
                *next += 1;
                if *next >= values.len() {
                    return OperationAction::Complete;
                }
                let Some(wait) = waits.get(*next).copied() else {
                    return Self::fail(5);
                };
                let Ok(sequence) = u32::try_from(*next) else {
                    return Self::fail(6);
                };
                let request = RequestId(sequence);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: BoundedValueRef::new(wait, 8).expect("sealed wait is eight bytes"),
                }
            }
            Self::Tee {
                value: Some(value),
                phase,
            } if *phase == 1 => {
                *phase = 2;
                OperationAction::Emit {
                    port: KernelPortId(1),
                    value: *value,
                }
            }
            Self::Tee { value, phase } if *phase == 2 => {
                *value = None;
                *phase = 0;
                OperationAction::Await
            }
            Self::Latest { closing, .. } if *closing => {
                *closing = false;
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    fn retains_resumed_value(&self) -> bool {
        matches!(
            self,
            Self::Latest {
                retain_resumed: true,
                ..
            }
        )
    }

    fn take_released_value(&mut self) -> Option<ValueRef> {
        match self {
            Self::Latest { released, .. } => released.take(),
            _ => None,
        }
    }

    fn cancel(&mut self) {
        if let Self::Latest {
            held,
            released,
            retain_resumed,
            ..
        } = self
        {
            *held = None;
            *released = None;
            *retain_resumed = false;
        }
    }
}

pub fn profile_catalog() -> ProfileCatalog {
    let mut catalog = ProfileCatalog::new();
    for definition in [
        definition(
            TICK_KIND,
            vec![],
            vec![port(TICK_PORT, PortDirection::Output)],
            vec![
                ConfigurationField {
                    key: "count".to_string(),
                    default_value: ConfigurationValue::U64(4),
                    validation: ConfigurationRule::U64Range {
                        minimum: 1,
                        maximum: 4,
                    },
                },
                ConfigurationField {
                    key: "period-ms".to_string(),
                    default_value: ConfigurationValue::U64(0),
                    validation: ConfigurationRule::U64Range {
                        minimum: 0,
                        maximum: u64::MAX,
                    },
                },
            ],
        ),
        definition(
            TEE_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![
                port(LEFT_PORT, PortDirection::Output),
                port(RIGHT_PORT, PortDirection::Output),
            ],
            vec![],
        ),
        definition(
            EVEN_FILTER_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![port(OUT_PORT, PortDirection::Output)],
            vec![],
        ),
        definition(
            LATEST_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![port(OUT_PORT, PortDirection::Output)],
            vec![],
        ),
        definition(
            SHOW_KIND,
            vec![port(IN_PORT, PortDirection::Input)],
            vec![],
            vec![],
        ),
    ] {
        catalog
            .insert(definition)
            .expect("multi-value profile kinds are unique");
    }
    catalog
}

pub fn advertisement(
    host_id: HostId,
    boot_id: conduit_core::BootId,
    offer_generation: OfferGeneration,
) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id,
        boot_id,
        offer_generation,
        profile: HostProfileId::from("conduit.std/kernel-multivalue@1"),
        resources: vec![
            resource_offer("std/kernel-presentation", PRESENTATION_RESOURCE_CLASS, 2),
            resource_offer("std/kernel-timer", TIMER_RESOURCE_CLASS, 1),
        ],
        capabilities: vec![
            offer(TICK_KIND, "time-tick", 1),
            offer(TEE_KIND, "flow-tee", 0),
            offer(EVEN_FILTER_KIND, "flow-filter-even", 0),
            offer(LATEST_KIND, "state-latest", 0),
            offer(SHOW_KIND, "presentation-show", 2),
        ],
    }
}

fn definition(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
    configuration: Vec<ConfigurationField>,
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: revision(kind),
        inputs,
        outputs,
        configuration,
    }
}

fn offer(kind: &str, capability: &str, resource_units: u32) -> CapabilityOffer {
    let definition = profile_catalog()
        .get(&kind_id(kind))
        .expect("offered multi-value kind exists")
        .clone();
    let host_operations = match kind {
        TICK_KIND => vec![wait_host_operation_requirement()],
        SHOW_KIND => vec![present_host_operation_requirement(
            kind_id("presentation/stdout-tick@1"),
            8,
        )],
        _ => vec![],
    };
    let resource_requirements = match kind {
        TICK_KIND => vec![resource_requirement(TIMER_RESOURCE_CLASS, resource_units)],
        SHOW_KIND => vec![resource_requirement(
            PRESENTATION_RESOURCE_CLASS,
            resource_units / 2,
        )],
        _ => vec![],
    };
    CapabilityOffer {
        capability_id: CapabilityId::from(capability),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        execution_profile_id: ExecutionProfileId::from(format!(
            "conduit.std/{capability}-kernel-hosted@1"
        )),
        implementation_id: conduit_core::ImplementationId::from(format!(
            "std/kernel-{capability}@1"
        )),
        artifact_id: ArtifactId::from(format!("conduit-std-host/{capability}@1")),
        inputs: definition.inputs,
        outputs: definition.outputs,
        host_operations,
        resource_requirements,
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: if kind == SHOW_KIND { 2 } else { 1 },
            max_queue_items: 4,
            max_queue_bytes: 64,
        },
    }
}

fn port(name: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(TICK_VALUE_KIND),
        direction,
    }
}

fn revision(kind: &str) -> KindContractRevision {
    KindContractRevision::from(format!("conduit.std/{kind}-tick@1"))
}

pub fn execute_fragment<W: Write, T: TimerAdapter>(
    host: &HostAdvertisement,
    fragment: &PlanFragment,
    activation_sequence: u64,
    next_evidence_sequence: &mut u64,
    output: &mut W,
    timer: &mut T,
) -> Result<MultiValueRunReport, String> {
    let lowered = lower_plan_fragment(fragment).map_err(|error| format!("lowering: {error:?}"))?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || lowered.cord_value_slots != QUEUE_SLOTS as u16
        || lowered
            .routes
            .iter()
            .map(|route| route.targets.len())
            .sum::<usize>()
            != ROUTE_TARGETS
        || lowered.host_operations.len() != 3
    {
        return Err("fragment does not match the installed multi-value kernel profile".to_string());
    }

    let find_node = |kind: &str, operation: Option<&str>| {
        lowered.nodes.iter().find(|node| {
            let placement = &fragment.placements[usize::from(node.node.0)];
            placement.kind_id.as_str() == kind
                && operation
                    .map(|name| placement.operation_id.as_str() == name)
                    .unwrap_or(true)
        })
    };
    let tick_node = find_node(TICK_KIND, None)
        .ok_or_else(|| "multi-value profile has no tick node".to_string())?
        .node;
    let tee_node = find_node(TEE_KIND, None)
        .ok_or_else(|| "multi-value profile has no tee node".to_string())?
        .node;
    let filter_node = find_node(EVEN_FILTER_KIND, None)
        .ok_or_else(|| "multi-value profile has no even-filter node".to_string())?
        .node;
    let latest_node = find_node(LATEST_KIND, None)
        .ok_or_else(|| "multi-value profile has no latest node".to_string())?
        .node;
    let show_even_node = find_node(SHOW_KIND, Some("show-even"))
        .ok_or_else(|| "multi-value profile has no show-even node".to_string())?
        .node;
    let show_latest_node = find_node(SHOW_KIND, Some("show-latest"))
        .ok_or_else(|| "multi-value profile has no show-latest node".to_string())?
        .node;
    let node_ids = [
        tick_node,
        tee_node,
        filter_node,
        latest_node,
        show_even_node,
        show_latest_node,
    ];
    if node_ids
        .iter()
        .enumerate()
        .any(|(index, node)| node_ids[..index].contains(node))
    {
        return Err("multi-value profile node roles are not distinct".to_string());
    }

    let tick_placement = &fragment.placements[usize::from(tick_node.0)];
    let count = configuration_u64(&tick_placement.configuration, "count")?;
    let period_ms = configuration_u64(&tick_placement.configuration, "period-ms")?;
    if count != 4 {
        return Err("installed multi-value conformance profile requires count = 4".to_string());
    }

    let mut values =
        HostedValueStore::new(8, 8, 64).map_err(|error| format!("multi-value store: {error:?}"))?;
    let mut tick_values = Vec::with_capacity(4);
    let mut wait_values = Vec::with_capacity(4);
    for tick in 0_u64..4 {
        tick_values.push(
            values
                .store(&tick.to_le_bytes())
                .map_err(|error| format!("preload tick: {error:?}"))?,
        );
        wait_values.push(
            values
                .store(&period_ms.to_le_bytes())
                .map_err(|error| format!("preload wait: {error:?}"))?,
        );
    }
    let value_allocation_before = values.allocation_capacities();

    let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal routes: {error:?}"))?;
    let mut host_bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(1);
    for operation in &lowered.host_operations {
        host_bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install host operation: {error:?}"))?;
    }
    host_bindings
        .seal()
        .map_err(|error| format!("seal host operations: {error:?}"))?;

    let mut operations: [Option<MultiValueOperation>; NODES] = [None, None, None, None, None, None];
    operations[usize::from(tick_node.0)] = Some(MultiValueOperation::Tick {
        values: tick_values.clone(),
        waits: wait_values,
        next: 0,
        pending: None,
    });
    operations[usize::from(tee_node.0)] = Some(MultiValueOperation::Tee {
        value: None,
        phase: 0,
    });
    operations[usize::from(filter_node.0)] = Some(MultiValueOperation::FilterEven {
        admitted: vec![tick_values[0], tick_values[2]],
    });
    operations[usize::from(latest_node.0)] = Some(MultiValueOperation::Latest {
        held: None,
        released: None,
        retain_resumed: false,
        closing: false,
    });
    operations[usize::from(show_even_node.0)] = Some(MultiValueOperation::Show {
        expected: vec![tick_values[0], tick_values[2]],
        next: 0,
        pending: None,
    });
    operations[usize::from(show_latest_node.0)] = Some(MultiValueOperation::Show {
        expected: vec![tick_values[3]],
        next: 0,
        pending: None,
    });
    let drivers: [OperationDriver<MultiValueOperation, PORTS>; NODES] = operations
        .map(|operation| {
            OperationDriver::new(
                operation.ok_or_else(|| "missing installed multi-value operation".to_string())?,
            )
            .map_err(|error| format!("prepare operation driver: {error:?}"))
        })
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "multi-value driver table width changed".to_string())?;
    let driver_capacity_before = drivers
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();

    let event_charge = u32::try_from(core::mem::size_of::<conduit_kernel::KernelEvent>())
        .map_err(|_| "kernel event charge overflow".to_string())?;
    let evidence = HostedEvidenceLog::new(256, event_charge.saturating_mul(256))
        .map_err(|error| format!("kernel evidence store: {error:?}"))?;
    let node_specs = lowered
        .node_specs
        .try_into()
        .map_err(|_| "multi-value node table width changed".to_string())?;
    let cord_specs = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "multi-value cord table width changed".to_string())?;
    let mut scheduler = MultiValueScheduler::new_with_host_operations(
        node_specs,
        cord_specs,
        routes,
        host_bindings,
        drivers,
        values,
        evidence,
    )
    .map_err(|error| format!("install multi-value scheduler: {error:?}"))?;

    let active_play = bind_active_play(
        &fragment.plan_id,
        &host.host_id,
        &host.boot_id,
        activation_sequence,
    );
    let show_even_placement = fragment.placements[usize::from(show_even_node.0)].clone();
    let show_latest_placement = fragment.placements[usize::from(show_latest_node.0)].clone();
    let mut receipts = Vec::with_capacity(3);
    let mut observations = Vec::with_capacity(4);
    let mut presentation_ids = Vec::with_capacity(3);
    loop {
        while let Some(request) = scheduler.next_host_request() {
            let encoded = scheduler
                .host_value(request.input.value)
                .map_err(|error| format!("read host-operation input: {error:?}"))?;
            if request.node == tick_node {
                let duration = encoded
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "wait input is not eight bytes".to_string())?;
                timer.wait(Duration::from_millis(duration));
            } else if request.node == show_even_node || request.node == show_latest_node {
                let tick = encoded
                    .try_into()
                    .map(u64::from_le_bytes)
                    .map_err(|_| "show input is not an eight-byte tick".to_string())?;
                let (branch, placement) = if request.node == show_even_node {
                    ("even", &show_even_placement)
                } else {
                    ("latest", &show_latest_placement)
                };
                let ordinal = receipts
                    .iter()
                    .filter(|receipt: &&MultiValueReceipt| {
                        receipt.placement_id == placement.placement_id
                    })
                    .count() as u64;
                let presentation = bind_presentation(
                    &active_play.active_play_id,
                    &placement.placement_id,
                    ordinal,
                );
                writeln!(output, "tick {branch} {tick}").map_err(|error| error.to_string())?;
                writeln!(
                    output,
                    "receipt tick placement={} branch={branch} value={tick}",
                    placement.placement_id.as_str()
                )
                .map_err(|error| error.to_string())?;
                receipts.push(MultiValueReceipt {
                    placement_id: placement.placement_id.clone(),
                    tick,
                });
                let evidence = bind_evidence(
                    &host.host_id,
                    &host.boot_id,
                    Some(&active_play.active_play_id),
                    *next_evidence_sequence,
                );
                *next_evidence_sequence = next_evidence_sequence
                    .checked_add(1)
                    .ok_or_else(|| "host evidence sequence exhausted".to_string())?;
                observations.push(Observation {
                    evidence_id: evidence.evidence_id,
                    active_play_id: Some(active_play.active_play_id.clone()),
                    presentation_id: Some(presentation.presentation_id.clone()),
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    plan_id: Some(fragment.plan_id.clone()),
                    placement_id: Some(placement.placement_id.clone()),
                    connection_id: fragment
                        .connections
                        .iter()
                        .find(|connection| connection.sink_placement_id == placement.placement_id)
                        .map(|connection| connection.connection_id.clone()),
                    kind: ObservationKind::ValuePresented {
                        value: ValuePayload {
                            value_kind: kind_id(TICK_VALUE_KIND),
                            encoded: tick.to_le_bytes().to_vec(),
                        },
                    },
                });
                presentation_ids.push(presentation.presentation_id);
            } else {
                return Err(format!(
                    "unmapped host request from node {}",
                    request.node.0
                ));
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
                .map_err(|error| format!("complete host operation: {error:?}"))?;
        }
        match scheduler
            .step()
            .map_err(|error| format!("kernel step: {error:?}"))?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => {
                return Err("multi-value kernel became idle before completion".to_string())
            }
            SchedulerStatus::Cancelled => {
                return Err("multi-value kernel was cancelled".to_string())
            }
        }
    }
    if receipts.len() != 3
        || receipts
            .iter()
            .map(|receipt| receipt.tick)
            .collect::<Vec<_>>()
            != [0, 2, 3]
        || scheduler.values().used_items() != 0
    {
        return Err(
            "multi-value kernel completed with incorrect receipts or retained values".to_string(),
        );
    }
    let driver_capacity_after = scheduler
        .drivers()
        .iter()
        .map(|driver| driver.operation().allocation_capacity())
        .sum::<usize>();
    if driver_capacity_after != driver_capacity_before {
        return Err("multi-value operation storage grew after activation".to_string());
    }
    let value_allocation_after = scheduler.values().allocation_capacities();
    if value_allocation_after != value_allocation_before {
        return Err("multi-value value storage grew after activation".to_string());
    }
    let terminal = bind_evidence(
        &host.host_id,
        &host.boot_id,
        Some(&active_play.active_play_id),
        *next_evidence_sequence,
    );
    *next_evidence_sequence = next_evidence_sequence
        .checked_add(1)
        .ok_or_else(|| "host evidence sequence exhausted".to_string())?;
    observations.push(Observation {
        evidence_id: terminal.evidence_id,
        active_play_id: Some(active_play.active_play_id.clone()),
        presentation_id: None,
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        plan_id: Some(fragment.plan_id.clone()),
        placement_id: None,
        connection_id: None,
        kind: ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed,
        },
    });

    Ok(MultiValueRunReport {
        observations,
        receipts,
        active_play_id: active_play.active_play_id,
        decisions: scheduler.decisions(),
        kernel_events: scheduler.evidence().len(),
        value_allocation_capacity_before: value_allocation_before,
        value_allocation_capacity_after: value_allocation_after,
        presentation_ids,
    })
}

fn configuration_u64(configuration: &[ConfigurationEntry], key: &str) -> Result<u64, String> {
    configuration
        .iter()
        .find(|entry| entry.key == key)
        .and_then(|entry| match entry.value {
            ConfigurationValue::U64(value) => Some(value),
            ConfigurationValue::Bool(_) => None,
        })
        .ok_or_else(|| format!("missing u64 configuration '{key}'"))
}

#[cfg(test)]
mod tests {
    use super::{advertisement, execute_fragment, profile_catalog};
    use crate::TimerAdapter;
    use conduit_core::{BootId, ConnectionProvider, HostId, OfferGeneration};
    use conduit_form::parse;
    use conduit_planner::{default_placements, plan};
    use conduit_runtime::lowering::lower_plan_fragment;
    use std::time::Duration;

    #[derive(Default)]
    struct VirtualTimer {
        waits: Vec<Duration>,
    }

    impl TimerAdapter for VirtualTimer {
        fn wait(&mut self, duration: Duration) {
            self.waits.push(duration);
        }
    }

    #[test]
    fn exact_multi_value_form_plans_and_lowers_all_numeric_tables() {
        let form = parse(
            include_str!("../../../examples/kernel-multivalue.form"),
            &profile_catalog(),
        )
        .expect("typed multi-value form parses");
        let host = advertisement(
            HostId::from("std-kernel-multivalue"),
            BootId::from("std-kernel-multivalue-boot"),
            OfferGeneration(1),
        );
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .expect("one exact capability exists per operation");
        let plan = plan(
            &form,
            core::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("typed multi-value form plans");
        let lowered = lower_plan_fragment(&plan.fragments[0]).expect("fragment lowers");
        assert_eq!(lowered.nodes.len(), 6);
        assert_eq!(lowered.cords.len(), 5);
        assert_eq!(lowered.routes.len(), 5);
        assert_eq!(lowered.host_operations.len(), 3);
        assert_eq!(lowered.resources.len(), 3);
        assert_eq!(lowered.cord_value_slots, 20);
        assert_eq!(lowered.cord_value_bytes, 320);
    }

    #[test]
    fn exact_multi_value_form_executes_real_host_operations_through_kernel() {
        let form = parse(
            include_str!("../../../examples/kernel-multivalue.form"),
            &profile_catalog(),
        )
        .expect("typed multi-value form parses");
        let host = advertisement(
            HostId::from("std-kernel-multivalue"),
            BootId::from("std-kernel-multivalue-boot"),
            OfferGeneration(1),
        );
        let placements = default_placements(&form, core::slice::from_ref(&host))
            .expect("one exact capability exists per operation");
        let plan = plan(
            &form,
            core::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("typed multi-value form plans");
        let mut output = Vec::new();
        let mut timer = VirtualTimer::default();
        let mut evidence_sequence = 0;
        let report = execute_fragment(
            &host,
            &plan.fragments[0],
            0,
            &mut evidence_sequence,
            &mut output,
            &mut timer,
        )
        .expect("multi-value kernel execution completes");

        assert_eq!(timer.waits, vec![Duration::ZERO; 4]);
        assert_eq!(
            report
                .receipts
                .iter()
                .map(|receipt| receipt.tick)
                .collect::<Vec<_>>(),
            [0, 2, 3]
        );
        let output = String::from_utf8(output).expect("output is utf-8");
        assert!(output.contains("tick even 0"));
        assert!(output.contains("tick even 2"));
        assert!(output.contains("tick latest 3"));
        assert!(report.decisions > 0);
        assert!(report.kernel_events > 0);
        assert_eq!(
            report.value_allocation_capacity_before,
            report.value_allocation_capacity_after
        );
        assert_eq!(report.presentation_ids.len(), 3);
        assert_eq!(report.observations.len(), 4);
        assert_eq!(evidence_sequence, 4);
    }
}
