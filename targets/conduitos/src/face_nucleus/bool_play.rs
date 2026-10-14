use alloc::{collections::BTreeMap, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, FACE_RESOURCE_CLASS, HostAdvertisement, HostId, HostProfileId,
    ImplementationId, InfoBool, KindIdentity, OfferGeneration, PROTOCOL_VERSION, Plan,
    PortDescriptor, PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutRect, MAX_GRAPHICS_SCENE_BYTES,
};
use conduit_kernel::scheduler::{CordSpec, FixedScheduler, HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore, HostCallDisposition,
    HostCallOutcome, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};
use conduit_plot::{ProfileCatalog, parse};

use super::back::FaceBack;
use crate::display::{DisplayError, DisplayReceipt, PixelTarget, render_scene};

const SOURCE_KIND: &str = "conduitos/fixture-bool-source";
const SOURCE_REVISION: &str = "conduitos/fixture-bool-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/bool-source@1";
const PLOT: &str = "plot bool_face {\n source: conduitos/fixture-bool-source\n show: face/bool(\"Current state\")\n source >> show\n}\n";
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 2;
const CORDS: usize = 1;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 2;
const MAX_VALUE_BYTES: usize = MAX_GRAPHICS_SCENE_BYTES;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 32;

type Scheduler = FixedScheduler<
    FaceBack,
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

pub struct PreparedBoolFace {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoolFaceProof {
    pub plan_id: conduit_core::PlanId,
    pub value: InfoBool,
    pub display: DisplayReceipt,
    pub kernel_signs: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoolFaceError {
    Catalog,
    Plot,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel,
    Value,
    Display(DisplayError),
}

pub fn prepare_bool(
    host: &str,
    boot: &str,
    value: InfoBool,
) -> Result<PreparedBoolFace, BoolFaceError> {
    let mut catalog = ProfileCatalog::new();
    conduit_semantic_catalog::install_bool_face_catalog(&mut catalog)
        .map_err(|_| BoolFaceError::Catalog)?;
    catalog
        .insert(conduit_plot::KindProjection {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindIdentity::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer(value).outputs,
            configuration: Default::default(),
        })
        .map_err(|_| BoolFaceError::Catalog)?;
    let plot = parse(PLOT, &catalog).map_err(|_| BoolFaceError::Plot)?;
    let advertisement = advertisement(host, boot, value);
    let hosts = [advertisement.clone()];
    let placements = default_placements(&plot, &hosts).map_err(|_| BoolFaceError::Placement)?;
    let plan = plan_with_options(
        &plot,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
    .map_err(|_| BoolFaceError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(BoolFaceError::Plan);
    }
    Ok(PreparedBoolFace {
        advertisement,
        plan,
    })
}

pub fn run_bool(
    prepared: &PreparedBoolFace,
    display: &mut impl PixelTarget,
) -> Result<BoolFaceProof, BoolFaceError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(BoolFaceError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| BoolFaceError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(BoolFaceError::Shape);
    }
    let source = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == SOURCE_KIND)
        .ok_or(BoolFaceError::Shape)?;
    let value = match source.capability_id.as_str() {
        "conduitos-fixture-bool-true@1" => InfoBool::TRUE,
        "conduitos-fixture-bool-false@1" => InfoBool::FALSE,
        _ => return Err(BoolFaceError::Shape),
    };
    let mut scheduler = scheduler(fragment, &lowered, value)?;
    let mut receipt = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|_| BoolFaceError::Value)?;
            let decoded = InfoBool::decode(input).map_err(|_| BoolFaceError::Value)?;
            if decoded != value || receipt.is_some() {
                return Err(BoolFaceError::Value);
            }
            receipt = Some(render(display, decoded)?);
            complete(&mut scheduler, request)?;
            continue;
        }
        match scheduler.step().map_err(|_| BoolFaceError::Kernel)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(BoolFaceError::Kernel);
            }
        }
    }
    Ok(BoolFaceProof {
        plan_id: prepared.plan.plan_id.clone(),
        value,
        display: receipt.ok_or(BoolFaceError::Shape)?,
        kernel_signs: lowered.sign_items,
    })
}

fn advertisement(host: &str, boot: &str, value: InfoBool) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(host),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("conduitos/two-lane-cooperative@1"),
        bases: vec![],
        resources: vec![resource_offer(
            &alloc::format!("{host}/display"),
            FACE_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![source_offer(value), bool_offer()],
    }
}

fn source_offer(value: InfoBool) -> CapabilityOffer {
    conduit_core::capability_offer_from_parts! {
        semantic_contract: Default::default(),
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(if value.get() {
            "conduitos-fixture-bool-true@1"
        } else {
            "conduitos-fixture-bool-false@1"
        }),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindIdentity::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(super::CONDUITOS_FACE_PROFILE),
            implementation_id: ImplementationId::from(SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(super::CONDUITOS_FACE_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
            abnormal_kind: None,
        }],
        host_calls: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
        },
    }
}

fn bool_offer() -> CapabilityOffer {
    super::face_nucleus_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::BOOL_FACE_KIND)
        .expect("ConduitOS Boolean presenter is installed")
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    value: InfoBool,
) -> Result<Scheduler, BoolFaceError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| BoolFaceError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| BoolFaceError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| BoolFaceError::Kernel)?;
    }
    routes.seal().map_err(|_| BoolFaceError::Kernel)?;
    let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| BoolFaceError::Kernel)?;
    }
    bindings.seal().map_err(|_| BoolFaceError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| BoolFaceError::Value)?;
    let drivers = fragment
        .placements
        .iter()
        .map(|placement| {
            let operation = if placement.kind_id.as_str() == SOURCE_KIND {
                FaceBack::Source {
                    value: values
                        .store(&value.encode())
                        .map_err(|_| BoolFaceError::Value)?,
                    emitted: false,
                }
            } else if placement.kind_id.as_str() == conduit_semantic_catalog::BOOL_FACE_KIND {
                FaceBack::Sink {
                    maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
                    pending: false,
                    complete: false,
                }
            } else {
                return Err(BoolFaceError::Shape);
            };
            Ok(operation)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| BoolFaceError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| BoolFaceError::Kernel)?;
    FixedScheduler::new_with_host_calls(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(|_| BoolFaceError::Kernel)
}

fn render(
    display: &mut impl PixelTarget,
    value: InfoBool,
) -> Result<DisplayReceipt, BoolFaceError> {
    let bounds = LayoutRect {
        x: 8,
        y: 8,
        width: 64,
        height: 16,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::text(
                bounds,
                bounds,
                GraphicsPaintRole::Foreground,
                if value.get() { "true" } else { "false" },
            )
            .map_err(|_| BoolFaceError::Value)?,
        )
        .map_err(|_| BoolFaceError::Value)?;
    render_scene(display, &scene).map_err(BoolFaceError::Display)
}

fn complete(scheduler: &mut Scheduler, request: HostCallRequest) -> Result<(), BoolFaceError> {
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: None,
                failure: None,
            },
        )
        .map_err(|_| BoolFaceError::Kernel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::{DisplayFormat, PixelTarget};

    struct Buffer {
        bytes: Vec<u8>,
        format: DisplayFormat,
        lost: bool,
    }

    impl Buffer {
        fn new() -> Self {
            let format = DisplayFormat {
                width: 96,
                height: 48,
                pitch: 96 * 4,
                bits_per_pixel: 32,
                red_shift: 16,
                green_shift: 8,
                blue_shift: 0,
            };
            Self {
                bytes: vec![0; format.byte_len().unwrap()],
                format,
                lost: false,
            }
        }
    }

    impl PixelTarget for Buffer {
        fn format(&self) -> DisplayFormat {
            self.format
        }

        fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
            if self.lost {
                return Err(DisplayError::Lost);
            }
            let offset = y as usize * self.format.pitch as usize + x as usize * 4;
            self.bytes[offset..offset + 4].copy_from_slice(&pixel.to_le_bytes());
            Ok(())
        }
    }

    #[test]
    fn ordinary_boolean_plot_plans_lowers_and_manifests_through_the_kernel() {
        for value in [InfoBool::FALSE, InfoBool::TRUE] {
            let prepared = prepare_bool("bool-host", "bool-boot", value).unwrap();
            let placement = prepared.plan.fragments[0]
                .placements
                .iter()
                .find(|placement| {
                    placement.kind_id.as_str() == conduit_semantic_catalog::BOOL_FACE_KIND
                })
                .unwrap();
            assert_eq!(
                placement.implementation_id.as_str(),
                "conduitos/face/bool-implementation@1"
            );
            let mut display = Buffer::new();
            let proof = run_bool(&prepared, &mut display).unwrap();
            assert_eq!(proof.value, value);
            assert_eq!(proof.display.commands, 1);
            assert!(proof.display.pixels_written > 0);
            assert!(display.bytes.iter().any(|byte| *byte != 0));
        }
    }

    #[test]
    fn display_loss_and_plan_identity_mutation_remain_fail_closed() {
        let prepared = prepare_bool("bool-host", "bool-boot", InfoBool::TRUE).unwrap();
        let mut display = Buffer::new();
        display.lost = true;
        assert_eq!(
            run_bool(&prepared, &mut display),
            Err(BoolFaceError::Display(DisplayError::Lost))
        );

        let mut mutated = prepared.plan.clone();
        mutated.fragments[0].placements[1].artifact_id = ArtifactId::from("mutated/bool");
        assert!(!conduit_core::verify_plan(&mutated));
    }
}
