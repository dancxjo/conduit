//! Bounded ordinary `logic/not` execution proof.

use alloc::{collections::BTreeMap, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionBase,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, InfoBool, KindContractRevision,
    OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection, PortTemporal, kind_id,
    port_id,
};
use conduit_form::{ProfileCatalog, StartupCatalog, parse};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    BoundedValueRef, FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};

use super::operation::PresentationOperation;
const SOURCE_KIND: &str = "conduitos/fixture-not-source";
const SOURCE_REVISION: &str = "conduitos/fixture-not-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/not-source@1";
const SINK_KIND: &str = "conduitos/fixture-not-sink";
const SINK_REVISION: &str = "conduitos/fixture-not-sink@1";
const SINK_IMPLEMENTATION: &str = "conduitos.fixture/not-sink@1";
const SINK_HOST_OPERATION: &str = "conduitos.fixture/capture-bool@1";
const FORM: &str = "form not_play {\n source: conduitos/fixture-not-source\n invert: logic/not\n sink: conduitos/fixture-not-sink\n source > invert\n invert > sink\n}\n";
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 3;
const CORDS: usize = 2;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 4;
const MAX_VALUE_BYTES: usize = conduit_core::BOOL_ENCODED_LEN;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 48;

type Kernel = FixedScheduler<
    OperationDriver<PresentationOperation, PORTS>,
    FixedValueStore<VALUES, MAX_VALUE_BYTES>,
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

pub struct PreparedLogicNot {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogicNotProof {
    pub plan_id: conduit_core::PlanId,
    pub input: InfoBool,
    pub output: InfoBool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LogicNotError {
    Catalog,
    Form,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel,
    Value,
}

struct Scheduler {
    kernel: Kernel,
    transform: NodeId,
    sink: NodeId,
}

pub fn prepare_not(
    host: &str,
    boot: &str,
    input: InfoBool,
) -> Result<PreparedLogicNot, LogicNotError> {
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_logic_catalogs(&mut StartupCatalog::new(), &mut catalog)
        .map_err(|_| LogicNotError::Catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer(input).outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| LogicNotError::Catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .map_err(|_| LogicNotError::Catalog)?;
    let form = parse(FORM, &catalog).map_err(|_| LogicNotError::Form)?;
    let advertisement = advertisement(host, boot, input);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| LogicNotError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::BOOL_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| LogicNotError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(LogicNotError::Plan);
    }
    Ok(PreparedLogicNot {
        advertisement,
        plan,
    })
}

pub fn run_not(prepared: &PreparedLogicNot) -> Result<LogicNotProof, LogicNotError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(LogicNotError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| LogicNotError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(LogicNotError::Shape);
    }
    let source = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == SOURCE_KIND)
        .ok_or(LogicNotError::Shape)?;
    let input = decode_source(source.capability_id.as_str())?;
    let mut scheduler = scheduler(fragment, &lowered, input)?;
    let mut output = None;
    let mut captured = None;
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            let bytes = scheduler
                .kernel
                .host_value(request.input.value)
                .map_err(|_| LogicNotError::Value)?;
            let value = InfoBool::decode(bytes).map_err(|_| LogicNotError::Value)?;
            if request.node == scheduler.transform && output.is_none() {
                let inverted = InfoBool::new(!value.get());
                complete_transform(&mut scheduler.kernel, request, inverted)?;
                output = Some(inverted);
            } else if request.node == scheduler.sink && output == Some(value) && captured.is_none()
            {
                captured = Some(value);
                complete_sink(&mut scheduler.kernel, request)?;
            } else {
                return Err(LogicNotError::Shape);
            }
            continue;
        }
        match scheduler.kernel.step().map_err(|_| LogicNotError::Kernel)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(LogicNotError::Kernel);
            }
        }
    }
    let output = output.ok_or(LogicNotError::Shape)?;
    if captured != Some(output) {
        return Err(LogicNotError::Shape);
    }
    Ok(LogicNotProof {
        plan_id: prepared.plan.plan_id.clone(),
        input,
        output,
    })
}

fn advertisement(host: &str, boot: &str, input: InfoBool) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        resources: Vec::new(),
        planner_capabilities: Vec::new(),
        capabilities: vec![
            source_offer(input),
            conduit_std_catalog::conduitos_logic_not_offer(),
            sink_offer(),
        ],
    }
}

fn source_offer(input: InfoBool) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(if input.get() {
            "conduitos-fixture-not-true@1"
        } else {
            "conduitos-fixture-not-false@1"
        }),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_LOGIC_NOT_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from(SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_LOGIC_NOT_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
        },
    }
}

fn sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-not-sink@1"),
        kind_id: kind_id(SINK_KIND),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_LOGIC_NOT_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from(SINK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_LOGIC_NOT_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::BOOL_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(SINK_HOST_OPERATION),
            target_kind: Some(kind_id(SINK_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
        },
    }
}

fn decode_source(capability: &str) -> Result<InfoBool, LogicNotError> {
    match capability {
        "conduitos-fixture-not-true@1" => Ok(InfoBool::TRUE),
        "conduitos-fixture-not-false@1" => Ok(InfoBool::FALSE),
        _ => Err(LogicNotError::Shape),
    }
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    input: InfoBool,
) -> Result<Scheduler, LogicNotError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| LogicNotError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| LogicNotError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| LogicNotError::Kernel)?;
    }
    routes.seal().map_err(|_| LogicNotError::Kernel)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| LogicNotError::Kernel)?;
    }
    bindings.seal().map_err(|_| LogicNotError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| LogicNotError::Value)?;
    let mut transform = None;
    let mut sink = None;
    let drivers = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let operation = match placement.kind_id.as_str() {
                SOURCE_KIND => PresentationOperation::Source {
                    value: values
                        .store(&input.encode())
                        .map_err(|_| LogicNotError::Value)?,
                    emitted: false,
                },
                conduit_std_catalog::LOGIC_NOT_KIND => {
                    transform = Some(NodeId(index as u16));
                    PresentationOperation::Transform {
                        maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
                        pending: false,
                        emitted: false,
                    }
                }
                SINK_KIND => {
                    sink = Some(NodeId(index as u16));
                    PresentationOperation::Sink {
                        maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
                        pending: false,
                        complete: false,
                    }
                }
                _ => return Err(LogicNotError::Shape),
            };
            OperationDriver::new(operation).map_err(|_| LogicNotError::Kernel)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| LogicNotError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| LogicNotError::Kernel)?;
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(|_| LogicNotError::Kernel)?;
    Ok(Scheduler {
        kernel,
        transform: transform.ok_or(LogicNotError::Shape)?,
        sink: sink.ok_or(LogicNotError::Shape)?,
    })
}

fn complete_transform(
    kernel: &mut Kernel,
    request: HostOperationRequest,
    output: InfoBool,
) -> Result<(), LogicNotError> {
    let value = kernel
        .store_host_value(&output.encode())
        .map_err(|_| LogicNotError::Value)?;
    let output = BoundedValueRef::new(value, conduit_core::BOOL_ENCODED_LEN as u32)
        .map_err(|_| LogicNotError::Value)?;
    kernel
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(output),
                failure: None,
            },
        )
        .map_err(|_| LogicNotError::Kernel)
}

fn complete_sink(kernel: &mut Kernel, request: HostOperationRequest) -> Result<(), LogicNotError> {
    kernel
        .complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|_| LogicNotError::Kernel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_not_form_inverts_both_values_through_the_production_kernel() {
        for input in [InfoBool::FALSE, InfoBool::TRUE] {
            let prepared = prepare_not("not-host", "not-boot", input).unwrap();
            let placement = prepared.plan.fragments[0]
                .placements
                .iter()
                .find(|p| p.kind_id.as_str() == conduit_std_catalog::LOGIC_NOT_KIND)
                .unwrap();
            assert_eq!(
                placement.implementation_id.as_str(),
                conduit_std_catalog::CONDUITOS_LOGIC_NOT_IMPLEMENTATION
            );
            let proof = run_not(&prepared).unwrap();
            assert_eq!(proof.output, InfoBool::new(!input.get()));
        }
    }

    #[test]
    fn mutated_plan_identity_is_refused() {
        let mut prepared = prepare_not("not-host", "not-boot", InfoBool::TRUE).unwrap();
        let transform = prepared.plan.fragments[0]
            .placements
            .iter_mut()
            .find(|p| p.kind_id.as_str() == conduit_std_catalog::LOGIC_NOT_KIND)
            .unwrap();
        transform.artifact_id = ArtifactId::from("mutated/not");
        assert!(!conduit_core::verify_plan(&prepared.plan));
    }
}
