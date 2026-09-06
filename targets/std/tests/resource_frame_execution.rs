#[path = "resource_common/execution.rs"]
mod execution;
use conduit_core::*;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, HostOperationDisposition, HostOperationOutcome,
    HostedSignLog, HostedValueStore, KernelEvent, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
use conduit_planner::proof::resource_frame::*;
use conduit_std_host::hosted_resource::HostedResourceGeneration;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
type Scheduler = FixedScheduler<
    OperationDriver<execution::FrameOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    4,
    3,
    PORTS,
    12,
    { 4 * PORTS },
    4,
    4,
    4,
>;

fn provider(
    binding: &ResourceBinding,
    read: &AuthorityGrantId,
    other: Option<&AuthorityGrantId>,
    writer: Option<&AuthorityGrantId>,
) -> HostedResourceGeneration {
    let c = &binding.content.as_ref().unwrap().contract;
    let reference = BoundedResourceRef {
        identity: c.identity,
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        extent: ResourceExtent {
            bytes: u64::from(FRAME_BYTES),
            items: Some(65536),
        },
        lifetime: ResourceLifetime {
            version: c.version,
            expires_at: None,
        },
    };
    let requirement = ResourceDereferenceRequirement {
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        authority_contract: AuthorityContractId::from(FRAME_AUTHORITY),
        maximum_bytes: u64::from(FRAME_BYTES),
        maximum_items: Some(65536),
    };
    let access = ResourceReferenceBinding {
        identity: reference.identity,
        version: reference.lifetime.version,
        content_profile: c.content_profile.clone(),
        access_class: binding.class_id.clone(),
        handle: ResourceHandleId::from(format!("handle/{}", binding.pool_id.as_str())),
        authority_contract: requirement.authority_contract.clone(),
        authority_grant: read.clone(),
        maximum_bytes: u64::from(FRAME_BYTES),
        maximum_items: Some(65536),
        availability: ResourceReferenceAvailability::Available,
    };
    let additional = other
        .map(|grant| {
            let mut a = access.clone();
            a.authority_grant = grant.clone();
            a
        })
        .into_iter()
        .collect();
    HostedResourceGeneration::new(binding, reference, requirement, access, writer.cloned())
        .unwrap()
        .with_readers(additional)
        .unwrap()
}

fn execute(copy: bool) -> (Vec<u64>, u16) {
    let planned = frame_resource_plan(copy, false).unwrap();
    let fragment = &planned.plan.fragments[0];
    let gear = |kind: &str| {
        fragment
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == kind)
            .unwrap()
    };
    let compose = gear("frame/compose");
    let display = gear("frame/display");
    let encoder = gear("frame/encoder");
    let grant = |p: &PlannedGear| p.authority[0].grant_id.clone();
    let writer = grant(compose);
    let display_grant = grant(display);
    let encoder_grant = grant(encoder);
    let input_binding = compose
        .resources
        .iter()
        .find(|r| r.class_id.as_str() == "resource/input-frame")
        .unwrap();
    let output_binding = compose
        .resources
        .iter()
        .find(|r| r.class_id.as_str() == "resource/frame")
        .unwrap();
    let mut owner = ResourceAdmissionOwner::new(planned.host.clone());
    let observations = planned
        .host
        .resources
        .iter()
        .map(|r| ResourceObservation {
            host_id: planned.host.host_id.clone(),
            boot_id: planned.host.boot_id.clone(),
            offer_generation: planned.host.offer_generation,
            pool_id: r.pool_id.clone(),
            class_id: r.class_id.clone(),
            health: ResourceHealth::Ready,
            unreserved_units: r.capacity_units,
            utilized_units: 0,
            sign_id: SignId::from(format!("sign/{}", r.pool_id.as_str())),
        })
        .collect::<Vec<_>>();
    for placement in &fragment.placements {
        owner
            .admit_planned_placement(planned.plan.plan_id.clone(), placement, &observations)
            .unwrap();
    }
    let mut input = provider(input_binding, &writer, None, None)
        .initialize(&vec![40; FRAME_BYTES as usize])
        .unwrap();
    let mut output = provider(
        output_binding,
        &display_grant,
        Some(&encoder_grant),
        Some(&writer),
    );
    let mut scratch = vec![0; FRAME_BYTES as usize];
    let mut copies = if copy {
        vec![vec![0; FRAME_BYTES as usize]; 2]
    } else {
        vec![]
    };
    let input_encoding = input.reference().encode().unwrap();
    let output_encoding = output.reference().encode().unwrap();
    let lowered = lower_plan_fragment(fragment).unwrap();
    let mut values = HostedValueStore::new(8, 512, 4096).unwrap();
    let input_value = values.store(&input.reference().encode().unwrap()).unwrap();
    let output_value = values.store(&output.reference().encode().unwrap()).unwrap();
    let mut routes = FixedRoutes::<{ 4 * PORTS }, 4>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .unwrap();
    }
    routes.seal().unwrap();
    let mut host = FixedHostOperationBindings::<4>::new(1);
    for operation in &lowered.host_operations {
        host.install(operation.node, operation.binding).unwrap();
    }
    host.seal().unwrap();
    let drivers = lowered
        .nodes
        .iter()
        .map(|node| {
            OperationDriver::new(execution::FrameOperation {
                input: node.inputs.first().map(|p| p.port),
                output: node.outputs.first().map(|p| {
                    (
                        p.port,
                        if p.value_kind == kind_id(RESOURCE_REFERENCE_INFO_ID)
                            && node.inputs.is_empty()
                        {
                            input_value
                        } else {
                            output_value
                        },
                    )
                }),
                operation: lowered
                    .host_operations
                    .iter()
                    .find(|o| o.node == node.node)
                    .map(|o| o.binding.operation),
            })
            .unwrap()
        })
        .collect::<Vec<_>>();
    let signs = HostedSignLog::new(256, 256 * std::mem::size_of::<KernelEvent>() as u32).unwrap();
    let mut scheduler = Scheduler::new_with_host_operations(
        lowered.node_specs.clone().try_into().ok().unwrap(),
        lowered
            .cords
            .iter()
            .map(|c| c.spec)
            .collect::<Vec<_>>()
            .try_into()
            .unwrap(),
        routes,
        host,
        drivers.try_into().ok().unwrap(),
        values,
        signs,
    )
    .unwrap();
    let mut checksums = Vec::with_capacity(2);
    let mut complete = false;
    for _ in 0..64 {
        if scheduler.step().unwrap() == SchedulerStatus::Complete {
            complete = true;
            break;
        }
        while let Some(request) = scheduler.next_host_request() {
            let placement_id = lowered.identity.placement_for_node(request.node).unwrap();
            let placement = fragment
                .placements
                .iter()
                .find(|p| &p.placement_id == placement_id)
                .unwrap();
            let received = scheduler.host_value(request.input.value).unwrap();
            let authority = &placement.authority[0];
            assert_eq!(
                authority.host_operation_contract_id.as_str(),
                FRAME_OPERATION
            );
            if placement.kind_id.as_str() == "frame/compose" {
                assert_eq!(received, input_encoding);
                let lease = input.acquire(&authority.grant_id).unwrap();
                for (value, result) in input.read(lease).unwrap().iter().zip(&mut scratch) {
                    *result = value + 2;
                }
                input.release(lease).unwrap();
                output
                    .write_candidate(&authority.grant_id, &scratch)
                    .unwrap();
                output.publish(&authority.grant_id).unwrap();
                scratch.fill(0);
            } else {
                assert_eq!(received, output_encoding);
                let lease = output.acquire(&authority.grant_id).unwrap();
                let bytes = output.read(lease).unwrap();
                let sum = if copy {
                    copies[checksums.len()].copy_from_slice(bytes);
                    copies[checksums.len()].iter().map(|b| u64::from(*b)).sum()
                } else {
                    bytes.iter().map(|b| u64::from(*b)).sum()
                };
                checksums.push(sum);
                output.release(lease).unwrap();
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
                .unwrap();
        }
    }
    assert!(complete);
    assert_eq!(checksums.len(), 2);
    assert_eq!(scheduler.pending_host_operation_count(), 0);
    let residencies = output.payload_residencies() + copies.len() as u16;
    output.retire(&writer).unwrap();
    for placement in &fragment.placements {
        owner
            .release(&planned.plan.plan_id, &placement.placement_id)
            .unwrap();
    }
    (checksums, residencies)
}
#[test]
fn ordinary_plans_and_kernel_execute_identical_frame_meaning_with_copy_or_shared_consumption() {
    let (copy, copy_residencies) = execute(true);
    let (shared, shared_residencies) = execute(false);
    assert_eq!(copy, shared);
    assert_eq!(shared, vec![42 * u64::from(FRAME_BYTES); 2]);
    assert_eq!(copy_residencies, 3);
    assert_eq!(shared_residencies, 1);
}
