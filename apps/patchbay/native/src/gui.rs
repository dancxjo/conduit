//! Small renderer-local Patchbay composition, geometry, and hit testing.

use crate::{
    canvas::SoftwareCanvas,
    canvas_viewport::CanvasViewport,
    gui_composition::{
        composition_port_point, draw_compositions, layout_compositions, CompositionLayout,
    },
    gui_gear::{draw_gear, GearViewContext},
    gui_gear_layout::{layout_gears, GearGeometry},
    gui_gesture::{draw_gesture, GestureView},
    gui_hit::HitShape,
    gui_inspector::{draw_inspector, InspectorView},
    gui_navigator::{draw_navigator, FormsNavigatorView},
    gui_primitives::{
        draw_regions, frame_rect, icon_label, line, positive, text, PixelRect, RegionMetrics,
    },
    icon::Icon,
    lifecycle_flow::{draw_lifecycle_flow, LifecycleFlow},
    parts_view::{draw_parts, PartsSelection},
};
use embedded_graphics::{
    draw_target::DrawTargetExt,
    pixelcolor::Rgb888,
    prelude::{DrawTarget, Point, Size},
    primitives::Rectangle,
};
use patchbay_model::{
    DebuggerPresentation, PatchbayGear, PatchbayGraph, PatchbayTheme, PHOSPHOR_THEME,
};

pub use crate::gui_hit::{GuiAction, HitTarget};

pub const MAX_HIT_TARGETS: usize = patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_GEARS
    + patchbay_model::MAX_PATCHBAY_PORTS
    + patchbay_model::MAX_PATCHBAY_PORTS
    + patchbay_model::MAX_PATCHBAY_CORDS
    + patchbay_model::MAX_PALETTE_ENTRIES
    + patchbay_model::MAX_PATCHBAY_GEARS * patchbay_model::MAX_FACE_CONTROLS * 2
    + 9
    + conduit_body::MAX_BODY_PARTS
    + conduit_body::MAX_CANDIDATES
    + 3
    + crate::forms_navigation::VISIBLE_FORM_ROWS
    + crate::lifecycle_flow::MAX_LIFECYCLE_ACTIONS;

pub(super) const HEADER_HEIGHT: i32 = 52;
pub(super) const FOOTER_HEIGHT: i32 = 42;
pub(super) const NAV_WIDTH: i32 = 176;
pub(super) const INSPECTOR_WIDTH: i32 = 284;
pub(super) const NODE_WIDTH: i32 = 190;
pub(super) const MINIMUM_NODE_HEIGHT: i32 = 92;

#[cfg(test)]
pub(super) use crate::gui_viewport::canvas_rect;
pub(super) use crate::gui_viewport::{canvas_world_bounds, subject_world_center};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LifecycleContext {
    pub body_id: Option<String>,
    pub wake_id: Option<String>,
    pub plan_id: Option<String>,
    pub play_id: Option<String>,
    pub flow: LifecycleFlow,
    pub parts: Option<patchbay_model::PartsView>,
    pub selected_part: Option<conduit_body::PartId>,
    pub selected_candidate: Option<conduit_body::CandidateId>,
    pub pending_revoke: Option<conduit_body::PartId>,
    pub browser_spawn_pending: bool,
    pub body_workbench_destination:
        Option<crate::native_body_workbench::NativeWorkbenchDestination>,
}

pub struct PatchbayViewContext<'a> {
    pub selected: Option<&'a str>,
    pub breadcrumb: &'a str,
    pub lifecycle: &'a LifecycleContext,
    pub palette: &'a crate::palette_state::PaletteChooser,
    pub forms: &'a [crate::forms_navigation::FormNavigatorEntry],
    pub form_selection: usize,
    pub form_scroll: usize,
    pub exact_identity_open: bool,
    pub face_control_focus: usize,
    pub presentation_layout: &'a patchbay_model::PatchbayLayout,
    pub realization_plan: Option<&'a conduit_core::Plan>,
    pub realization_hosts: &'a [conduit_core::HostAdvertisement],
    pub status: Option<&'a crate::interaction_status::InteractionStatus>,
    pub gesture: GestureView<'a>,
    pub viewport: &'a CanvasViewport,
}

#[derive(Clone)]
pub(super) struct GearLayout<'a> {
    pub(super) gear: &'a PatchbayGear,
    pub(super) bounds: PixelRect,
    pub(super) inputs: Vec<(String, Point)>,
    pub(super) outputs: Vec<(String, Point)>,
    pub(super) group: Option<String>,
}

#[derive(Clone)]
pub(super) struct BoundaryLayout {
    pub(super) identity: String,
    pub(super) point: Point,
    pub(super) bounds: PixelRect,
    pub(super) label: String,
    pub(super) is_output: bool,
}

pub fn draw_patchbay(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    graph: &PatchbayGraph,
    view: PatchbayViewContext<'_>,
) -> Vec<HitTarget> {
    draw_patchbay_with_debugger(pixels, width, height, graph, view, None)
}

pub fn draw_patchbay_with_debugger(
    pixels: &mut [u32],
    width: usize,
    height: usize,
    graph: &PatchbayGraph,
    view: PatchbayViewContext<'_>,
    debugger: Option<&DebuggerPresentation>,
) -> Vec<HitTarget> {
    let PatchbayViewContext {
        selected,
        breadcrumb,
        lifecycle,
        palette,
        forms,
        form_selection,
        form_scroll,
        exact_identity_open,
        face_control_focus,
        presentation_layout,
        realization_plan,
        realization_hosts,
        status,
        gesture,
        viewport,
    } = view;
    debug_assert!(Icon::ALL
        .iter()
        .all(|icon| !icon.accessibility_name().is_empty()));
    let mut canvas = SoftwareCanvas::new(pixels, width, height);
    let theme = &PHOSPHOR_THEME;
    let width = i32::try_from(width).unwrap_or(i32::MAX);
    let height = i32::try_from(height).unwrap_or(i32::MAX);
    let inspector_requested = selected.is_some() || palette.search_active() || exact_identity_open;
    let shell = u16::try_from(width)
        .ok()
        .zip(u16::try_from(height).ok())
        .and_then(|(width, height)| {
            patchbay_model::ResponsivePatchbayLayout::allocate(
                width,
                height,
                100,
                inspector_requested,
            )
            .ok()
        });
    let shell_region = |id| shell.as_ref().and_then(|layout| layout.region(id));
    let header_height = shell_region(patchbay_model::PresentationRegionId::HeaderMeaning)
        .map_or(HEADER_HEIGHT, |region| i32::from(region.bounds.height));
    let footer_height = shell_region(patchbay_model::PresentationRegionId::FooterMeaning)
        .map_or(FOOTER_HEIGHT, |region| i32::from(region.bounds.height));
    let nav_width = shell_region(patchbay_model::PresentationRegionId::Navigator)
        .map_or(NAV_WIDTH, |region| i32::from(region.bounds.width));
    let inspector_width = shell_region(patchbay_model::PresentationRegionId::Inspector)
        .map_or(0, |region| i32::from(region.bounds.width));
    let mut targets = Vec::with_capacity(MAX_HIT_TARGETS);
    draw_regions(
        &mut canvas,
        width,
        height,
        RegionMetrics {
            header_height,
            footer_height,
            nav_width,
            inspector_width,
        },
        theme,
    );
    {
        let clip = Rectangle::new(
            Point::zero(),
            Size::new(
                u32::try_from(width).unwrap_or_default(),
                positive(header_height),
            ),
        );
        let mut header = canvas.clipped(&clip);
        draw_header(
            &mut header,
            graph,
            breadcrumb,
            (lifecycle, width, viewport),
            theme,
            &mut targets,
        );
    }
    let mut compositions = layout_compositions(graph, width);
    let mut layouts = layout_gears(
        graph,
        width,
        presentation_layout,
        GearGeometry {
            canvas_left: nav_width + 28,
            inspector_width,
            header_height,
            node_width: NODE_WIDTH,
            minimum_node_height: MINIMUM_NODE_HEIGHT,
        },
    );
    let mut boundaries = layout_boundaries(graph, width);
    crate::gui_viewport::transform_canvas_layout(
        viewport,
        &mut layouts,
        &mut compositions,
        &mut boundaries,
    );
    {
        let clip = Rectangle::new(
            Point::new(0, header_height),
            Size::new(
                positive(nav_width),
                positive(height - header_height - footer_height),
            ),
        );
        let mut navigator = canvas.clipped(&clip);
        draw_navigator(
            &mut navigator,
            palette,
            graph.gears.len() + graph.compositions.len(),
            FormsNavigatorView {
                entries: forms,
                selection: form_selection,
                scroll: form_scroll,
                body_born: lifecycle.body_id.is_some(),
                parts_open: lifecycle.parts.is_some(),
                body_workbench_destination: lifecycle.body_workbench_destination,
            },
            theme,
            &mut targets,
        );
    }
    {
        let clip = viewport.canvas();
        let clip = Rectangle::new(
            Point::new(clip.x, clip.y),
            Size::new(clip.width, clip.height),
        );
        let mut canvas = canvas.clipped(&clip);
        if let Some(parts) = &lifecycle.parts {
            draw_parts(
                &mut canvas,
                parts,
                PartsSelection {
                    part: lifecycle.selected_part.as_ref(),
                    candidate: lifecycle.selected_candidate.as_ref(),
                    pending_revoke: lifecycle.pending_revoke.as_ref(),
                    browser_spawn_pending: lifecycle.browser_spawn_pending,
                },
                viewport.canvas(),
                theme,
                &mut targets,
            );
        } else {
            draw_cords(
                &mut canvas,
                graph,
                (&layouts, &compositions, &boundaries),
                selected,
                (presentation_layout, viewport),
                theme,
                &mut targets,
            );
            draw_boundaries(&mut canvas, graph, &boundaries, theme, &mut targets);
            draw_compositions(&mut canvas, graph, &compositions, theme, &mut targets);
            let gear_view = GearViewContext {
                presentation_layout,
                realization_plan,
                realization_hosts,
                face_control_focus,
            };
            for layout in &layouts {
                draw_gear(
                    &mut canvas,
                    graph,
                    layout,
                    selected,
                    &gear_view,
                    theme,
                    &mut targets,
                );
            }
            if let Some(debugger) = debugger {
                crate::gui_debugger::draw_debugger_overlay(
                    &mut canvas,
                    graph,
                    (&layouts, &compositions, &boundaries),
                    (presentation_layout, viewport),
                    debugger,
                    theme,
                );
            }
            draw_gesture(
                &mut canvas,
                graph,
                &layouts,
                &compositions,
                &boundaries,
                &gesture,
                theme,
            );
        }
    }
    if inspector_width > 0 {
        let clip = Rectangle::new(
            Point::new(width - inspector_width, header_height),
            Size::new(
                positive(inspector_width),
                positive(height - header_height - footer_height),
            ),
        );
        let mut inspector = canvas.clipped(&clip);
        draw_inspector(
            &mut inspector,
            graph,
            InspectorView {
                selected,
                palette,
                lifecycle,
                status,
                exact_open: exact_identity_open,
                width,
                inspector_width,
            },
            theme,
            &mut targets,
        );
    }
    draw_footer(
        &mut canvas,
        graph,
        FooterView {
            selected,
            status,
            viewport,
            width,
            height,
            footer_height,
        },
        theme,
        &mut targets,
    );
    targets.truncate(MAX_HIT_TARGETS);
    targets
}

fn draw_header<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    breadcrumb: &str,
    view: (&LifecycleContext, i32, &CanvasViewport),
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let (lifecycle, width, viewport) = view;
    {
        let clip = Rectangle::new(Point::zero(), Size::new(232, HEADER_HEIGHT as u32));
        let mut meaning = target.clipped(&clip);
        icon_label(
            &mut meaning,
            Icon::Form,
            Point::new(14, 10),
            &format!(
                "FORM  {}",
                if breadcrumb.is_empty() {
                    graph.form_name.as_str()
                } else {
                    breadcrumb
                }
            ),
            theme.emphasis,
        );
    }
    draw_lifecycle_flow(target, lifecycle, width, theme, targets);
    let _ = viewport;
}

fn draw_cords<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    layout: (
        &[GearLayout<'_>],
        &[CompositionLayout<'_>],
        &[BoundaryLayout],
    ),
    selected: Option<&str>,
    presentation: (&patchbay_model::PatchbayLayout, &CanvasViewport),
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    for cord in &graph.cords {
        let Some(points) = cord_route_points(cord, layout, presentation) else {
            continue;
        };
        let color = if selected == Some(cord.identity.as_str()) {
            theme.focus
        } else {
            theme.structure_primary
        };
        for segment in points.windows(2) {
            line(target, segment[0], segment[1], color);
        }
        targets.push(HitTarget {
            action: select_action(graph, &cord.identity),
            shape: HitShape::Cord { points },
        });
    }
}

pub(super) fn cord_route_points(
    cord: &patchbay_model::PatchbayCord,
    layout: (
        &[GearLayout<'_>],
        &[CompositionLayout<'_>],
        &[BoundaryLayout],
    ),
    presentation: (&patchbay_model::PatchbayLayout, &CanvasViewport),
) -> Option<[Point; 5]> {
    let (layouts, compositions, boundaries) = layout;
    let (presentation_layout, viewport) = presentation;
    let source = find_port(layouts, compositions, boundaries, &cord.source_port)?;
    let sink = find_port(layouts, compositions, boundaries, &cord.sink_port)?;
    let default_x = source.x + (sink.x - source.x) / 2;
    let (bend_x, bend_y) = presentation_layout
        .cord_route(&cord.source_port, &cord.sink_port)
        .and_then(|(x, y)| viewport.world_to_screen(Point::new(x, y)).ok())
        .map(|point| (point.x, point.y))
        .unwrap_or((default_x, source.y + (sink.y - source.y) / 2));
    Some([
        source,
        Point::new(bend_x, source.y),
        Point::new(bend_x, bend_y),
        Point::new(sink.x, bend_y),
        sink,
    ])
}

fn select_action(graph: &PatchbayGraph, identity: &str) -> GuiAction {
    GuiAction::SelectSubject(
        graph
            .subject_ref(identity)
            .expect("drawn subject belongs to the exact graph"),
    )
}

pub(super) fn find_port(
    layouts: &[GearLayout<'_>],
    compositions: &[CompositionLayout<'_>],
    boundaries: &[BoundaryLayout],
    identity: &str,
) -> Option<Point> {
    layouts
        .iter()
        .flat_map(|layout| layout.inputs.iter().chain(&layout.outputs))
        .find_map(|(candidate, point)| (candidate == identity).then_some(*point))
        .or_else(|| composition_port_point(compositions, identity))
        .or_else(|| {
            boundaries
                .iter()
                .find_map(|boundary| (boundary.identity == identity).then_some(boundary.point))
        })
}

pub(super) fn layout_boundaries(graph: &PatchbayGraph, width: i32) -> Vec<BoundaryLayout> {
    let left = NAV_WIDTH + 8;
    let right = (width - INSPECTOR_WIDTH - 8).max(left + 80);
    graph
        .face_inputs
        .iter()
        .enumerate()
        .map(|(index, port)| {
            let y = HEADER_HEIGHT + 42 + index as i32 * 34;
            BoundaryLayout {
                identity: port.identity.clone(),
                point: Point::new(left + 10, y + 10),
                bounds: PixelRect {
                    x: left,
                    y,
                    width: 82,
                    height: 22,
                },
                label: format!("> {}", port.descriptor.port_id.as_str()),
                is_output: false,
            }
        })
        .chain(graph.face_outputs.iter().enumerate().map(|(index, port)| {
            let y = HEADER_HEIGHT + 42 + index as i32 * 34;
            BoundaryLayout {
                identity: port.identity.clone(),
                point: Point::new(right - 10, y + 10),
                bounds: PixelRect {
                    x: right - 82,
                    y,
                    width: 82,
                    height: 22,
                },
                label: format!("{} >", port.descriptor.port_id.as_str()),
                is_output: true,
            }
        }))
        .collect()
}

fn draw_boundaries<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    boundaries: &[BoundaryLayout],
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    for boundary in boundaries {
        frame_rect(target, boundary.bounds, theme.focus, 1);
        text(
            target,
            Point::new(boundary.bounds.x + 7, boundary.bounds.y + 7),
            &boundary.label,
            theme.text_primary,
        );
        targets.push(HitTarget {
            action: select_action(graph, &boundary.identity),
            shape: HitShape::Rect(boundary.bounds),
        });
    }
}

struct FooterView<'a> {
    selected: Option<&'a str>,
    status: Option<&'a crate::interaction_status::InteractionStatus>,
    viewport: &'a CanvasViewport,
    width: i32,
    height: i32,
    footer_height: i32,
}

fn draw_footer<D: DrawTarget<Color = Rgb888>>(
    target: &mut D,
    graph: &PatchbayGraph,
    view: FooterView<'_>,
    theme: &PatchbayTheme,
    targets: &mut Vec<HitTarget>,
) {
    let FooterView {
        selected,
        status,
        viewport,
        width,
        height,
        footer_height,
    } = view;
    let y = height - footer_height + 12;
    icon_label(
        target,
        Icon::Success,
        Point::new(14, y - 4),
        "CHECKED",
        theme.success,
    );
    text(
        target,
        Point::new(132, y),
        &format!(
            "Gears {}  Ports {}  Cords {}",
            graph.gears.len(),
            graph
                .gears
                .iter()
                .map(|g| g.inputs.len() + g.outputs.len())
                .sum::<usize>(),
            graph.cords.len()
        ),
        theme.text_primary,
    );
    if let Some(status) = status {
        use crate::interaction_status::InteractionStatusLevel;
        let (icon, label, color) = match status.level {
            InteractionStatusLevel::Success => (Icon::Success, "SUCCESS", theme.success),
            InteractionStatusLevel::Information => (Icon::Info, "INFO", theme.focus),
            InteractionStatusLevel::Refusal => (Icon::Warning, "REFUSED", theme.emphasis),
            InteractionStatusLevel::Failure => (Icon::Failure, "FAILED", theme.failure),
        };
        icon_label(target, icon, Point::new(400, y - 4), label, color);
        text(target, Point::new(500, y), &status.text, color);
    } else if selected.is_some() {
        text(
            target,
            Point::new(430, y),
            "selection is presentation-only",
            theme.text_secondary,
        );
    } else if width >= 720 {
        crate::gui_viewport::draw_viewport_controls(
            target,
            viewport,
            Point::new(width.saturating_sub(252), height - footer_height + 4),
            theme,
            targets,
        );
    }
}
