use alloc::{collections::BTreeMap, vec, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId, InfoBool,
    KindContractRevision, OfferGeneration, PRESENTATION_RESOURCE_CLASS, PROTOCOL_VERSION, Plan,
    PortDescriptor, PortDirection, PortTemporal, kind_id, port_id, resource_offer,
};
use conduit_form::{ProfileCatalog, parse};
use conduit_kernel::scheduler::{
    CordSpec, FixedScheduler, HostOperationRequest, OperationDriver, SchedulerStatus,
};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};
use conduit_planner::{PlanningOptions, default_placements, plan_with_options};
use conduit_presentation::{
    GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutRect, MAX_GRAPHICS_SCENE_BYTES,
};

use super::operation::PresentationOperation;
use crate::display::{DisplayError, DisplayReceipt, PixelTarget, render_scene};

const SOURCE_KIND: &str = "conduitos/fixture-bool-source";
const SOURCE_REVISION: &str = "conduitos/fixture-bool-source@1";
const SOURCE_IMPLEMENTATION: &str = "conduitos.fixture/bool-source@1";
const FORM: &str = "form bool_presentation {\n source: conduitos/fixture-bool-source\n show: presentation/bool\n source > show\n}\n";
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

pub struct PreparedBoolPresentation {
    pub advertisement: HostAdvertisement,
    pub plan: Plan,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoolPresentationProof {
    pub plan_id: conduit_core::PlanId,
    pub value: InfoBool,
    pub display: DisplayReceipt,
    pub kernel_signs: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoolPresentationError {
    Catalog,
    Form,
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
) -> Result<PreparedBoolPresentation, BoolPresentationError> {
    let mut catalog = ProfileCatalog::new();
    conduit_std_catalog::install_bool_presentation_catalog(&mut catalog)
        .map_err(|_| BoolPresentationError::Catalog)?;
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: kind_id(SOURCE_KIND),
            kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
            inputs: Vec::new(),
            outputs: source_offer(value).outputs,
            configuration: Vec::new(),
        })
        .map_err(|_| BoolPresentationError::Catalog)?;
    let form = parse(FORM, &catalog).map_err(|_| BoolPresentationError::Form)?;
    let advertisement = advertisement(host, boot, value);
    let hosts = [advertisement.clone()];
    let placements =
        default_placements(&form, &hosts).map_err(|_| BoolPresentationError::Placement)?;
    let plan = plan_with_options(
        &form,
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
    .map_err(|_| BoolPresentationError::Plan)?;
    if !conduit_core::verify_plan(&plan) || plan.fragments.len() != 1 {
        return Err(BoolPresentationError::Plan);
    }
    Ok(PreparedBoolPresentation {
        advertisement,
        plan,
    })
}

pub fn run_bool(
    prepared: &PreparedBoolPresentation,
    display: &mut impl PixelTarget,
) -> Result<BoolPresentationProof, BoolPresentationError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(BoolPresentationError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| BoolPresentationError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(BoolPresentationError::Shape);
    }
    let source = fragment
        .placements
        .iter()
        .find(|placement| placement.kind_id.as_str() == SOURCE_KIND)
        .ok_or(BoolPresentationError::Shape)?;
    let value = match source.capability_id.as_str() {
        "conduitos-fixture-bool-true@1" => InfoBool::TRUE,
        "conduitos-fixture-bool-false@1" => InfoBool::FALSE,
        _ => return Err(BoolPresentationError::Shape),
    };
    let mut scheduler = scheduler(fragment, &lowered, value)?;
    let mut receipt = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|_| BoolPresentationError::Value)?;
            let decoded = InfoBool::decode(input).map_err(|_| BoolPresentationError::Value)?;
            if decoded != value || receipt.is_some() {
                return Err(BoolPresentationError::Value);
            }
            receipt = Some(render(display, decoded)?);
            complete(&mut scheduler, request)?;
            continue;
        }
        match scheduler
            .step()
            .map_err(|_| BoolPresentationError::Kernel)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(BoolPresentationError::Kernel);
            }
        }
    }
    Ok(BoolPresentationProof {
        plan_id: prepared.plan.plan_id.clone(),
        value,
        display: receipt.ok_or(BoolPresentationError::Shape)?,
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
        resources: vec![resource_offer(
            &alloc::format!("{host}/display"),
            PRESENTATION_RESOURCE_CLASS,
            1,
        )],
        planner_capabilities: Vec::new(),
        capabilities: vec![source_offer(value), bool_offer()],
    }
}

fn source_offer(value: InfoBool) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(if value.get() {
            "conduitos-fixture-bool-true@1"
        } else {
            "conduitos-fixture-bool-false@1"
        }),
        kind_id: kind_id(SOURCE_KIND),
        kind_contract_revision: KindContractRevision::from(SOURCE_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(super::CONDUITOS_PRESENTATION_PROFILE),
            implementation_id: ImplementationId::from(SOURCE_IMPLEMENTATION),
            artifact_id: ArtifactId::from(super::CONDUITOS_PRESENTATION_ARTIFACT),
        },
        inputs: Vec::new(),
        outputs: vec![PortDescriptor {
            port_id: port_id("value"),
            value_kind: kind_id(conduit_core::BOOL_INFO_ID),
            direction: PortDirection::Output,
            temporal: PortTemporal::Current,
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

fn bool_offer() -> CapabilityOffer {
    super::presentation_nucleus_offers()
        .into_iter()
        .find(|offer| offer.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND)
        .expect("ConduitOS Boolean presenter is installed")
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    value: InfoBool,
) -> Result<Scheduler, BoolPresentationError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| BoolPresentationError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| BoolPresentationError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| BoolPresentationError::Kernel)?;
    }
    routes.seal().map_err(|_| BoolPresentationError::Kernel)?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| BoolPresentationError::Kernel)?;
    }
    bindings.seal().map_err(|_| BoolPresentationError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| BoolPresentationError::Value)?;
    let drivers = fragment
        .placements
        .iter()
        .map(|placement| {
            let operation = if placement.kind_id.as_str() == SOURCE_KIND {
                PresentationOperation::Source {
                    value: values
                        .store(&value.encode())
                        .map_err(|_| BoolPresentationError::Value)?,
                    emitted: false,
                }
            } else if placement.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND {
                PresentationOperation::Sink {
                    maximum_input_bytes: conduit_core::BOOL_ENCODED_LEN as u32,
                    pending: false,
                    complete: false,
                }
            } else {
                return Err(BoolPresentationError::Shape);
            };
            OperationDriver::new(operation).map_err(|_| BoolPresentationError::Kernel)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| BoolPresentationError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| BoolPresentationError::Kernel)?;
    FixedScheduler::new_with_host_operations(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(|_| BoolPresentationError::Kernel)
}

fn render(
    display: &mut impl PixelTarget,
    value: InfoBool,
) -> Result<DisplayReceipt, BoolPresentationError> {
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
            .map_err(|_| BoolPresentationError::Value)?,
        )
        .map_err(|_| BoolPresentationError::Value)?;
    render_scene(display, &scene).map_err(BoolPresentationError::Display)
}

fn complete(
    scheduler: &mut Scheduler,
    request: HostOperationRequest,
) -> Result<(), BoolPresentationError> {
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
        .map_err(|_| BoolPresentationError::Kernel)
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
    fn ordinary_boolean_form_plans_lowers_and_manifests_through_the_kernel() {
        for value in [InfoBool::FALSE, InfoBool::TRUE] {
            let prepared = prepare_bool("bool-host", "bool-boot", value).unwrap();
            let placement = prepared.plan.fragments[0]
                .placements
                .iter()
                .find(|placement| {
                    placement.kind_id.as_str() == conduit_std_catalog::BOOL_PRESENTATION_KIND
                })
                .unwrap();
            assert_eq!(
                placement.implementation_id.as_str(),
                "conduitos/presentation/bool-implementation@1"
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
            Err(BoolPresentationError::Display(DisplayError::Lost))
        );

        let mut mutated = prepared.plan.clone();
        mutated.fragments[0].placements[1].artifact_id = ArtifactId::from("mutated/bool");
        assert!(!conduit_core::verify_plan(&mutated));
    }
}
