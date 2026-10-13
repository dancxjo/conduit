use alloc::{string::String, vec::Vec};
use conduit_face::{
    FaceComposition, GraphicsCommand, GraphicsPaintRole, GraphicsScene, LayoutFrame, LayoutRect,
    MAX_GRAPHICS_SCENE_BYTES,
};
use conduit_kernel::scheduler::{FixedScheduler, HostCallRequest, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, FixedHostCallBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostCallDisposition, HostCallOutcome, ValueStorage,
};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

use super::{PreparedFacePlay, TEXT_SOURCE_KIND, back::FaceBack};
use crate::display::{DisplayReceipt, PixelTarget, render_scene};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 11;
const CORDS: usize = 8;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 32;
const MAX_VALUE_BYTES: usize = MAX_GRAPHICS_SCENE_BYTES;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 256;

type FaceScheduler = FixedScheduler<
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FaceProof {
    pub plan_id: conduit_core::PlanId,
    pub fragment_id: conduit_core::FragmentId,
    pub text: String,
    pub layout_children: u8,
    pub graphics_commands: u8,
    pub text_display: DisplayReceipt,
    pub display: DisplayReceipt,
    pub kernel_signs: u16,
    pub realization_back: conduit_core::PlotBack,
    pub node_count: u8,
    pub cord_count: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FaceRunError {
    Shape,
    Kernel,
    Value,
    Transform,
    Text,
    Layout,
    Graphics,
    Display(crate::display::DisplayError),
    MissingManifestation,
}

impl FaceRunError {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shape => "face-plan-shape-invalid",
            Self::Kernel => "face-kernel-failed",
            Self::Value => "face-value-invalid",
            Self::Transform => "face-transform-failed",
            Self::Text => "face-text-invalid",
            Self::Layout => "face-layout-invalid",
            Self::Graphics => "face-graphics-invalid",
            Self::Display(error) => error.as_str(),
            Self::MissingManifestation => "face-manifestation-missing",
        }
    }
}

pub fn run(
    prepared: &PreparedFacePlay,
    display: &mut impl PixelTarget,
) -> Result<FaceProof, FaceRunError> {
    let fragment = prepared.plan.fragments.first().ok_or(FaceRunError::Shape)?;
    let [realization_back] = prepared.plan.realization_backs.as_slice() else {
        return Err(FaceRunError::Shape);
    };
    if realization_back.kind_id.as_str() != conduit_semantic_catalog::PATCHBAY_GEAR_FACE_KIND
        || realization_back.invocation_path != "conduitos-gear-front/front"
        || fragment.realization_backs != prepared.plan.realization_backs
    {
        return Err(FaceRunError::Shape);
    }
    let mut scheduler = prepare_scheduler(fragment, &prepared.lowered)?;
    let mut text = None;
    let mut text_display_receipt = None;
    let mut layout_children = None;
    let mut graphics_commands = None;
    let mut display_receipt = None;
    loop {
        if let Some(request) = scheduler.next_host_request() {
            let placement = fragment
                .placements
                .get(usize::from(request.node.0))
                .ok_or(FaceRunError::Shape)?;
            let input = scheduler
                .host_value(request.input.value)
                .map_err(|_| FaceRunError::Value)?
                .to_vec();
            match placement.kind_id.as_str() {
                conduit_semantic_catalog::TEXT_FACE_KIND => {
                    if text.is_some() {
                        return Err(FaceRunError::Text);
                    }
                    let value = String::from_utf8(input).map_err(|_| FaceRunError::Text)?;
                    text_display_receipt = Some(render_text(display, &value)?);
                    text = Some(value);
                    complete(&mut scheduler, request, None)?;
                }
                conduit_semantic_catalog::GRAPHICS_FACE_KIND => {
                    if display_receipt.is_some() {
                        return Err(FaceRunError::Graphics);
                    }
                    let scene =
                        GraphicsScene::decode(&input).map_err(|_| FaceRunError::Graphics)?;
                    graphics_commands = Some(
                        u8::try_from(scene.commands().len()).map_err(|_| FaceRunError::Graphics)?,
                    );
                    display_receipt =
                        Some(render_scene(display, &scene).map_err(FaceRunError::Display)?);
                    complete(&mut scheduler, request, None)?;
                }
                _ => {
                    let output = transform(placement, &input)?;
                    let terminal_layout =
                        placement.kind_id.as_str() == conduit_semantic_catalog::LAYOUT_COLUMN_KIND;
                    if terminal_layout {
                        let frame =
                            LayoutFrame::decode(&output).map_err(|_| FaceRunError::Layout)?;
                        layout_children = Some(frame.child_count);
                    }
                    let bounded = if terminal_layout {
                        None
                    } else {
                        let value = scheduler
                            .store_host_value(&output)
                            .map_err(|_| FaceRunError::Value)?;
                        let maximum = placement
                            .host_calls
                            .first()
                            .ok_or(FaceRunError::Shape)?
                            .maximum_output_bytes;
                        Some(
                            BoundedValueRef::new(value, maximum)
                                .map_err(|_| FaceRunError::Value)?,
                        )
                    };
                    complete(&mut scheduler, request, bounded)?;
                }
            }
            continue;
        }
        match scheduler.step().map_err(|_| FaceRunError::Kernel)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Drained => break,
            SchedulerStatus::Idle | SchedulerStatus::Cancelled => {
                return Err(FaceRunError::Kernel);
            }
        }
    }
    Ok(FaceProof {
        plan_id: prepared.plan.plan_id.clone(),
        fragment_id: fragment.fragment_id.clone(),
        text: text.ok_or(FaceRunError::MissingManifestation)?,
        layout_children: layout_children.ok_or(FaceRunError::MissingManifestation)?,
        graphics_commands: graphics_commands.ok_or(FaceRunError::MissingManifestation)?,
        text_display: text_display_receipt.ok_or(FaceRunError::MissingManifestation)?,
        display: display_receipt.ok_or(FaceRunError::MissingManifestation)?,
        kernel_signs: prepared.lowered.sign_items,
        realization_back: realization_back.clone(),
        node_count: u8::try_from(fragment.placements.len()).map_err(|_| FaceRunError::Shape)?,
        cord_count: u8::try_from(fragment.connections.len()).map_err(|_| FaceRunError::Shape)?,
    })
}

fn render_text(display: &mut impl PixelTarget, text: &str) -> Result<DisplayReceipt, FaceRunError> {
    let bounds = LayoutRect {
        x: 8,
        y: 120,
        width: 160,
        height: 16,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::text(bounds, bounds, GraphicsPaintRole::Foreground, text)
                .map_err(|_| FaceRunError::Text)?,
        )
        .map_err(|_| FaceRunError::Text)?;
    render_scene(display, &scene).map_err(FaceRunError::Display)
}

fn prepare_scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<FaceScheduler, FaceRunError> {
    if fragment.placements.len() != NODES
        || fragment.connections.len() != CORDS
        || lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(FaceRunError::Shape);
    }
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| FaceRunError::Shape)?;
    let cords = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| FaceRunError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| FaceRunError::Kernel)?;
    }
    routes.seal().map_err(|_| FaceRunError::Kernel)?;
    let mut bindings = FixedHostCallBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_calls {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| FaceRunError::Kernel)?;
    }
    bindings.seal().map_err(|_| FaceRunError::Kernel)?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| FaceRunError::Value)?;
    let mut drivers = Vec::with_capacity(NODES);
    for placement in &fragment.placements {
        let operation = match placement.kind_id.as_str() {
            conduit_semantic_catalog::LAYOUT_VIEWPORT_KIND => {
                let value = conduit_semantic_catalog::execute_layout_source(placement)
                    .map_err(|_| FaceRunError::Transform)?;
                let encoded = value.encode();
                source(&mut values, &encoded[..value.encoded_len()])?
            }
            conduit_semantic_catalog::FACE_ICON_KIND => {
                let value = conduit_semantic_catalog::execute_face_source(placement)
                    .map_err(|_| FaceRunError::Transform)?;
                let encoded = value.encode();
                source(&mut values, &encoded[..value.encoded_len()])?
            }
            TEXT_SOURCE_KIND => source(&mut values, b"Gear Front")?,
            conduit_semantic_catalog::TEXT_FACE_KIND
            | conduit_semantic_catalog::GRAPHICS_FACE_KIND
            | conduit_semantic_catalog::LAYOUT_COLUMN_KIND => FaceBack::Sink {
                maximum_input_bytes: placement
                    .host_calls
                    .first()
                    .ok_or(FaceRunError::Shape)?
                    .maximum_input_bytes,
                pending: false,
                complete: false,
            },
            _ => FaceBack::Transform {
                maximum_input_bytes: placement
                    .host_calls
                    .first()
                    .ok_or(FaceRunError::Shape)?
                    .maximum_input_bytes,
                pending: false,
                emitted: false,
            },
        };
        drivers.push(operation);
    }
    let drivers = drivers.try_into().map_err(|_| FaceRunError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        lowered
            .sign_bytes
            .max((SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32),
    )
    .map_err(|_| FaceRunError::Kernel)?;
    FixedScheduler::new_with_host_calls(nodes, cords, routes, bindings, drivers, values, signs)
        .map_err(|_| FaceRunError::Kernel)
}

fn source(
    values: &mut FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    bytes: &[u8],
) -> Result<FaceBack, FaceRunError> {
    Ok(FaceBack::Source {
        value: values.store(bytes).map_err(|_| FaceRunError::Value)?,
        emitted: false,
    })
}

fn transform(placement: &conduit_core::PlannedGear, input: &[u8]) -> Result<Vec<u8>, FaceRunError> {
    match placement.kind_id.as_str() {
        conduit_semantic_catalog::LAYOUT_INSET_KIND
        | conduit_semantic_catalog::LAYOUT_ROW_KIND
        | conduit_semantic_catalog::LAYOUT_COLUMN_KIND
        | conduit_semantic_catalog::LAYOUT_STACK_KIND
        | conduit_semantic_catalog::LAYOUT_ALIGN_KIND => {
            let frame = LayoutFrame::decode(input).map_err(|_| FaceRunError::Layout)?;
            let output = conduit_semantic_catalog::execute_layout_transform(placement, frame)
                .map_err(|_| FaceRunError::Transform)?;
            Ok(output.encode()[..output.encoded_len()].to_vec())
        }
        conduit_semantic_catalog::FACE_FRAME_KIND | conduit_semantic_catalog::FACE_BADGE_KIND => {
            let value = FaceComposition::decode(input).map_err(|_| FaceRunError::Transform)?;
            let output = conduit_semantic_catalog::execute_face_transform(placement, value)
                .map_err(|_| FaceRunError::Transform)?;
            Ok(output.encode()[..output.encoded_len()].to_vec())
        }
        conduit_semantic_catalog::GRAPHICS_RECT_KIND => {
            let value = FaceComposition::decode(input).map_err(|_| FaceRunError::Graphics)?;
            encode_scene(
                conduit_semantic_catalog::execute_graphics_transform(placement, Some(value), None)
                    .map_err(|_| FaceRunError::Transform)?,
            )
        }
        conduit_semantic_catalog::GRAPHICS_TEXT_KIND
        | conduit_semantic_catalog::GRAPHICS_ICON_KIND => {
            let scene = GraphicsScene::decode(input).map_err(|_| FaceRunError::Graphics)?;
            encode_scene(
                conduit_semantic_catalog::execute_graphics_transform(placement, None, Some(scene))
                    .map_err(|_| FaceRunError::Transform)?,
            )
        }
        _ => Err(FaceRunError::Transform),
    }
}

fn encode_scene(scene: GraphicsScene) -> Result<Vec<u8>, FaceRunError> {
    Ok(scene.encode()[..scene.encoded_len()].to_vec())
}

fn complete(
    scheduler: &mut FaceScheduler,
    request: HostCallRequest,
    output: Option<BoundedValueRef>,
) -> Result<(), FaceRunError> {
    scheduler
        .complete_host_call(
            request.node,
            request.request,
            HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output,
                failure: None,
            },
        )
        .map_err(|_| FaceRunError::Kernel)
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::display::{DisplayError, DisplayFormat};
    use alloc::vec;

    fn with_kernel_stack(test: impl FnOnce() + Send + 'static) {
        // Debug builds retain large by-value fixed-scheduler temporaries.
        // Keep the test budget explicit as the admitted scene bound grows.
        std::thread::Builder::new()
            .stack_size(8 * 1024 * 1024)
            .spawn(test)
            .unwrap()
            .join()
            .unwrap();
    }

    struct Buffer {
        format: DisplayFormat,
        bytes: Vec<u8>,
        lost: bool,
    }

    impl Buffer {
        fn new() -> Self {
            Self {
                format: DisplayFormat {
                    width: 320,
                    height: 200,
                    pitch: 1280,
                    bits_per_pixel: 32,
                    red_shift: 16,
                    green_shift: 8,
                    blue_shift: 0,
                },
                bytes: vec![0; 320 * 200 * 4],
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
    fn one_ordinary_plot_runs_all_three_branches_through_the_kernel() {
        with_kernel_stack(|| {
            let prepared = super::super::prepare("test-host", "test-boot").unwrap();
            let mut display = Buffer::new();
            let proof = run(&prepared, &mut display).unwrap();
            assert_eq!(proof.text, "Gear Front");
            assert_eq!(proof.layout_children, 3);
            assert_eq!(proof.graphics_commands, 3);
            assert_eq!(proof.text_display.commands, 1);
            assert!(proof.text_display.pixels_written > 0);
            assert_eq!(proof.display.commands, 3);
            assert!(proof.display.pixels_written > 0);
            assert!(display.bytes.iter().any(|byte| *byte != 0));
        });
    }

    #[test]
    fn display_loss_is_not_kernel_or_semantic_success() {
        with_kernel_stack(|| {
            let prepared = super::super::prepare("test-host", "test-boot").unwrap();
            let mut display = Buffer::new();
            display.lost = true;
            assert_eq!(
                run(&prepared, &mut display),
                Err(FaceRunError::Display(DisplayError::Lost))
            );
        });
    }
}
