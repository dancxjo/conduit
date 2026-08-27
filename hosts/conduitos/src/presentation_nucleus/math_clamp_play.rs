//! Bounded ordinary `math/clamp` execution proof.

use alloc::{collections::BTreeMap, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostOperationContractId,
    HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision,
    OfferGeneration, PROTOCOL_VERSION, Plan, PortDescriptor, PortDirection, PortTemporal, Scalar,
    kind_id, port_id,
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
const SOURCE_KIND: &str = "conduitos/fixture-clamp-source";
const SOURCE_REVISION: &str = "conduitos/fixture-clamp-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/clamp-source@1";
const SINK_KIND: &str = "conduitos/fixture-clamp-sink";
const SINK_REVISION: &str = "conduitos/fixture-clamp-sink@1";
const SINK_IMPLEMENTATION: &str = "conduitos.fixture/clamp-sink@1";
const SINK_HOST_OPERATION: &str = "conduitos.fixture/capture-scalar@1";
const FORM: &str = "form clamp_play {\n source: conduitos/fixture-clamp-source\n clamp: math/clamp(minimum = -1000000, maximum = 1000000)\n sink: conduitos/fixture-clamp-sink\n source > clamp\n clamp > sink\n}\n";
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 3;
const CORDS: usize = 2;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 4;
const MAX_VALUE_BYTES: usize = conduit_core::SCALAR_ENCODED_LEN;
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

pub struct PreparedMathClamp {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MathClampProof {
    pub plan_id: conduit_core::PlanId,
    pub input: Scalar,
    pub output: Scalar,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MathClampError {
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

pub fn prepare_clamp(
    host: &str,
    boot: &str,
    input: Scalar,
) -> Result<PreparedMathClamp, MathClampError> {
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_math_catalogs(&mut StartupCatalog::new(), &mut catalog)
        .map_err(|_| MathClampError::Catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer(input).outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| MathClampError::Catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SINK_KIND),
            kind_contract_revision: KindContractRevision::from(SINK_REVISION),
            inputs: sink_offer().inputs,
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .map_err(|_| MathClampError::Catalog)?;
    let form = parse(FORM, &catalog).map_err(|_| MathClampError::Form)?;
    let advertisement = advertisement(host, boot, input);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&form, &hosts).map_err(|_| MathClampError::Placement)?;
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_core::SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .map_err(|_| MathClampError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(MathClampError::Plan);
    }
    Ok(PreparedMathClamp {
        advertisement,
        plan,
    })
}

pub fn run_clamp(prepared: &PreparedMathClamp) -> Result<MathClampProof, MathClampError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(MathClampError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| MathClampError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(MathClampError::Shape);
    }
    let source = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == SOURCE_KIND)
        .ok_or(MathClampError::Shape)?;
    let input = decode_source(source.capability_id.as_str())?;
    let clamp = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == conduit_std_catalog::MATH_CLAMP_KIND)
        .ok_or(MathClampError::Shape)?;
    let minimum = configured_scalar(clamp, conduit_std_catalog::CLAMP_MINIMUM_KEY)?;
    let maximum = configured_scalar(clamp, conduit_std_catalog::CLAMP_MAXIMUM_KEY)?;
    let mut scheduler = scheduler(fragment, &lowered, input)?;
    let mut output = None;
    let mut captured = None;
    loop {
        if let Some(request) = scheduler.kernel.next_host_request() {
            let bytes = scheduler
                .kernel
                .host_value(request.input.value)
                .map_err(|_| MathClampError::Value)?;
            let value = Scalar::decode(bytes).map_err(|_| MathClampError::Value)?;
            if request.node == scheduler.transform && output.is_none() {
                let clamped = conduit_std_catalog::clamp_scalar(value, minimum, maximum)
                    .map_err(|_| MathClampError::Value)?;
                complete_transform(&mut scheduler.kernel, request, clamped)?;
                output = Some(clamped);
            } else if request.node == scheduler.sink && output == Some(value) && captured.is_none()
            {
                captured = Some(value);
                complete_sink(&mut scheduler.kernel, request)?;
            } else {
                return Err(MathClampError::Shape);
            }
            continue;
        }
        match scheduler
            .kernel
            .step()
            .map_err(|_| MathClampError::Kernel)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(MathClampError::Kernel);
            }
        }
    }
    let output = output.ok_or(MathClampError::Shape)?;
    if captured != Some(output) {
        return Err(MathClampError::Shape);
    }
    Ok(MathClampProof {
        plan_id: prepared.plan.plan_id.clone(),
        input,
        output,
    })
}

fn advertisement(host: &str, boot: &str, input: Scalar) -> HostAdvertisement {
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
            conduit_std_catalog::conduitos_math_clamp_offer(),
            sink_offer(),
        ],
    }
}

fn source_offer(input: Scalar) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(alloc::format!(
            "conduitos-fixture-clamp-{}@1",
            input.raw_microunits()
        )),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_MATH_CLAMP_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from(SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_MATH_CLAMP_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value,
        }],
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        },
    }
}

fn sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("conduitos-fixture-clamp-sink@1"),
        kind_id: kind_id(SINK_KIND),
        kind_contract_revision: KindContractRevision::from(SINK_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(
                conduit_std_catalog::CONDUITOS_MATH_CLAMP_EXECUTION_PROFILE,
            ),
            implementation_id: ImplementationId::from(SINK_IMPLEMENTATION),
            artifact_id: ArtifactId::from(conduit_std_catalog::CONDUITOS_MATH_CLAMP_ARTIFACT),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::SCALAR_INFO_ID),
            direction: PortDirection::Input,
            temporal: PortTemporal::Value,
        }],
        outputs: Vec::new(),
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(SINK_HOST_OPERATION),
            target_kind: Some(kind_id(SINK_KIND)),
            maximum_in_flight: 1,
            maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
            maximum_output_bytes: 0,
        }],
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
        },
    }
}

fn decode_source(capability: &str) -> Result<Scalar, MathClampError> {
    capability
        .strip_prefix("conduitos-fixture-clamp-")
        .and_then(|value| value.strip_suffix("@1"))
        .and_then(|value| value.parse::<i64>().ok())
        .map(Scalar::from_raw_microunits)
        .ok_or(MathClampError::Shape)
}

fn configured_scalar(
    placement: &conduit_core::PlannedGear,
    key: &str,
) -> Result<Scalar, MathClampError> {
    placement
        .configuration
        .iter()
        .find_map(|entry| match (&*entry.key, &entry.value) {
            (candidate, conduit_core::ConfigurationValue::I64(value)) if candidate == key => {
                Some(Scalar::from_raw_microunits(*value))
            }
            _ => None,
        })
        .ok_or(MathClampError::Shape)
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    input: Scalar,
) -> Result<Scheduler, MathClampError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| MathClampError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| MathClampError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| MathClampError::Kernel)?;
    }
    routes.seal().map_err(|_| MathClampError::Kernel)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| MathClampError::Kernel)?;
    }
    bindings.seal().map_err(|_| MathClampError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| MathClampError::Value)?;
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
                        .map_err(|_| MathClampError::Value)?,
                    emitted: false,
                },
                conduit_std_catalog::MATH_CLAMP_KIND => {
                    transform = Some(NodeId(index as u16));
                    PresentationOperation::Transform {
                        maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
                        pending: false,
                        emitted: false,
                    }
                }
                SINK_KIND => {
                    sink = Some(NodeId(index as u16));
                    PresentationOperation::Sink {
                        maximum_input_bytes: conduit_core::SCALAR_ENCODED_LEN as u32,
                        pending: false,
                        complete: false,
                    }
                }
                _ => return Err(MathClampError::Shape),
            };
            OperationDriver::new(operation).map_err(|_| MathClampError::Kernel)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| MathClampError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| MathClampError::Kernel)?;
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(|_| MathClampError::Kernel)?;
    Ok(Scheduler {
        kernel,
        transform: transform.ok_or(MathClampError::Shape)?,
        sink: sink.ok_or(MathClampError::Shape)?,
    })
}

fn complete_transform(
    kernel: &mut Kernel,
    request: HostOperationRequest,
    output: Scalar,
) -> Result<(), MathClampError> {
    let value = kernel
        .store_host_value(&output.encode())
        .map_err(|_| MathClampError::Value)?;
    let output = BoundedValueRef::new(value, conduit_core::SCALAR_ENCODED_LEN as u32)
        .map_err(|_| MathClampError::Value)?;
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
        .map_err(|_| MathClampError::Kernel)
}

fn complete_sink(kernel: &mut Kernel, request: HostOperationRequest) -> Result<(), MathClampError> {
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
        .map_err(|_| MathClampError::Kernel)
}
