//! Existing Conduit Face scene and typography, without environmental reconstruction.
pub(crate) const STATUS_STRIP_HEIGHT: usize = 36;
use crate::raster_target::{ChartViewport, Pixels, paint_scene};
use conduit_face::{Face, FaceBasis, FaceRole, FaceSubject, FaceText};
use conduitos::{display::DisplayFormat, native_face_scene::NativeFaceScene};

/// Host-owned pre-birth status, never represented as the environmental Body Face.
pub fn unavailable(reason: &str) -> Face {
    Face::new(
        0,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "local/display-status".into(),
            role: FaceRole::Document,
            name: "Conduit".into(),
        }],
        vec![],
        vec![],
        vec![FaceText {
            subject: "local/display-status".into(),
            text: reason.into(),
        }],
    )
    .expect("bounded static host status")
}

/// The adapter supplies ordinary Mask-local guidance. Choose the first wording
/// admitted by the shared measured footer; never clip text or ignore refusal.
fn set_read_only_notice(
    scene: &mut NativeFaceScene,
    missing_glyphs: bool,
) -> Result<(), conduitos::native_face_scene::FaceSceneError> {
    use conduitos::native_face_scene::FaceSceneError;
    let candidates = if missing_glyphs {
        [
            "Font missing characters; original text retained",
            "Missing glyphs; text retained",
            "Missing glyphs",
        ]
    } else {
        [
            "Read only; Details for actions and facts",
            "Read only; see Details",
            "Read only; Details",
        ]
    };
    for notice in candidates {
        match scene.set_local_notice(notice) {
            Ok(()) => return Ok(()),
            Err(FaceSceneError::InputBound) => continue,
            Err(error) => return Err(error),
        }
    }
    Err(FaceSceneError::InputBound)
}

pub struct Presentation<'a> {
    pub unit: Option<conduit_core::Unit>,
    pub look: Option<&'a conduit_face::LookInfo>,
    pub details: bool,
    /// Properties are historical facts of the retained snapshot, not current liveness.
    pub retained_snapshot: bool,
    pub history_in_primary: bool,
}
pub fn render_with_palette(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    pixels: &mut [u32],
    preferred: Option<conduit_core::Unit>,
    look: Option<&conduit_face::LookInfo>,
) -> Result<(usize, usize), String> {
    render_with_details(
        face,
        width,
        height,
        page_tick,
        pixels,
        Presentation {
            unit: preferred,
            look,
            details: false,
            retained_snapshot: false,
            history_in_primary: true,
        },
    )
}
pub fn render_with_details(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    pixels: &mut [u32],
    presentation: Presentation<'_>,
) -> Result<(usize, usize), String> {
    let Presentation {
        unit: preferred,
        look,
        details,
        retained_snapshot,
        history_in_primary,
    } = presentation;
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("raster extent mismatch".into());
    }
    let (frame, page, pages, scale, chart_viewport) = frame_with_context(
        face,
        width,
        height,
        page_tick,
        preferred,
        details,
        look,
        retained_snapshot,
        history_in_primary,
    )?;
    if chart_viewport {
        render_graphics_with_palette(&frame, width, height, pixels, look)?;
        return Ok((page, pages));
    }
    let mut target = Pixels {
        format: DisplayFormat {
            width: width.into(),
            height: height.into(),
            pitch: u32::from(width) * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        },
        values: pixels,
    };
    paint_scene(&mut target, &frame, scale, look)
        .map_err(|e| format!("Face raster refused: {e:?}"))?;
    Ok((page, pages))
}

#[cfg(test)]
pub fn render_graphics(
    scene: &conduit_face::GraphicsScene,
    width: u16,
    height: u16,
    pixels: &mut [u32],
) -> Result<(), String> {
    render_graphics_with_palette(scene, width, height, pixels, None)
}
pub fn render_graphics_with_palette(
    scene: &conduit_face::GraphicsScene,
    width: u16,
    height: u16,
    pixels: &mut [u32],
    look: Option<&conduit_face::LookInfo>,
) -> Result<(), String> {
    if pixels.len() != usize::from(width) * usize::from(height)
        || usize::from(height) <= STATUS_STRIP_HEIGHT
        || width == 0
    {
        return Err("raster extent mismatch".into());
    }
    pixels.fill(0);
    let mut target = ChartViewport {
        width: usize::from(width),
        height: usize::from(height),
        pixels,
    };
    paint_scene(&mut target, scene, 100, look)
        .map(|_| ())
        .map_err(|e| format!("history raster: {e:?}"))
}
/// Render already prepared native-resolution chart pixels one-to-one, reserving
/// a separate Host status strip. No glyph or line pixel is stretched.
/// Host presentation status is separate from the unchanged producer Face.
pub fn render_notice(
    text: &str,
    width: u16,
    height: u16,
    pixels: &mut [u32],
) -> Result<(), String> {
    render_notice_with_palette(text, width, height, pixels, None)
}
pub fn render_notice_with_palette(
    text: &str,
    width: u16,
    height: u16,
    pixels: &mut [u32],
    look: Option<&conduit_face::LookInfo>,
) -> Result<(), String> {
    use conduit_face::{
        GraphicsCommand, GraphicsPaintRole, GraphicsScene, GraphicsShapeStyle, LayoutRect,
    };
    let clip = LayoutRect {
        x: 0,
        y: 0,
        width,
        height,
    };
    let mut scene = GraphicsScene::empty();
    scene
        .push(
            GraphicsCommand::rect(
                LayoutRect {
                    x: 0,
                    y: 0,
                    width,
                    height: 30,
                },
                clip,
                GraphicsPaintRole::Background,
                GraphicsShapeStyle::Fill,
            )
            .map_err(|e| format!("notice: {e:?}"))?,
        )
        .map_err(|e| format!("notice: {e:?}"))?;
    let bounded: String = text.chars().take(90).collect();
    scene
        .push(
            GraphicsCommand::text(
                LayoutRect {
                    x: 8,
                    y: 4,
                    width: width.saturating_sub(16),
                    height: 24,
                },
                clip,
                GraphicsPaintRole::Foreground,
                &bounded,
            )
            .map_err(|e| format!("notice: {e:?}"))?,
        )
        .map_err(|e| format!("notice: {e:?}"))?;
    let mut target = Pixels {
        format: DisplayFormat {
            width: width.into(),
            height: height.into(),
            pitch: u32::from(width) * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        },
        values: pixels,
    };
    paint_scene(&mut target, &scene, 100, look)
        .map(|_| ())
        .map_err(|e| format!("notice raster: {e:?}"))
}

#[cfg(test)]
pub(crate) fn frame(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    preferred: Option<conduit_core::Unit>,
) -> Result<(conduit_face::GraphicsScene, usize, usize, u16, bool), String> {
    frame_with_details(face, width, height, page_tick, preferred, false)
}
pub(crate) fn frame_with_details(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    preferred: Option<conduit_core::Unit>,
    details: bool,
) -> Result<(conduit_face::GraphicsScene, usize, usize, u16, bool), String> {
    frame_with_look(face, width, height, page_tick, preferred, details, None)
}
pub(crate) fn frame_with_look(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    preferred: Option<conduit_core::Unit>,
    details: bool,
    look: Option<&conduit_face::LookInfo>,
) -> Result<(conduit_face::GraphicsScene, usize, usize, u16, bool), String> {
    frame_with_context(
        face, width, height, page_tick, preferred, details, look, false, true,
    )
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn frame_with_context(
    face: Face,
    width: u16,
    height: u16,
    page_tick: usize,
    preferred: Option<conduit_core::Unit>,
    details: bool,
    look: Option<&conduit_face::LookInfo>,
    retained_snapshot: bool,
    history_in_primary: bool,
) -> Result<(conduit_face::GraphicsScene, usize, usize, u16, bool), String> {
    let property_face = face.clone();
    let property = crate::property_mask::frame_with_context(
        &property_face,
        width,
        height,
        0,
        preferred,
        look,
        retained_snapshot,
    );
    let mut scene = if let Some(look) = look {
        let admitted = conduit_face::admit_visual_look(
            look,
            conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 100,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .map_err(|e| format!("Look capacity refused: {e:?}"))?;
        NativeFaceScene::prepare_visual_look(face, width, height, false, &admitted)
    } else {
        NativeFaceScene::prepare_with_input(face, width, height, false)
    }
    .map_err(|e| format!("Face scene refused: {e:?}"))?;
    scene
        .use_read_only_overview()
        .map_err(|e| format!("overview: {e:?}"))?;
    scene.show_details(details);
    let unsupported = property_face
        .subjects
        .iter()
        .flat_map(|subject| subject.name.chars())
        .any(|character| {
            conduitos::display::typography::glyph(
                conduitos::display::typography::TextRole::Body,
                character,
            )
            .unsupported
        });
    set_read_only_notice(&mut scene, unsupported)
        .map_err(|e| format!("Face notice refused: {e:?}"))?;
    if details {
        let pages = scene.pages();
        let page = page_tick % pages;
        for _ in 0..page {
            scene.turn_page(true);
        }
        let frame = scene.frame().map_err(|e| format!("Details: {e:?}"))?;
        return Ok((frame.scene, page, pages, scene.text_scale_percent(), false));
    }
    let summarized = crate::source_history::scope(&property_face)?;
    let shared = if history_in_primary {
        crate::source_history::prepare(
            &property_face,
            preferred.unwrap_or(conduit_core::Unit::Celsius),
            width,
            height.saturating_sub(STATUS_STRIP_HEIGHT as u16),
            None,
            look,
        )?
    } else {
        None
    };
    let shared_pages = usize::from(shared.is_some());
    let charts: Vec<_> = property_face
        .subjects
        .iter()
        .filter_map(|subject| {
            conduit_environment_history::HistoryGraph::prepare(&property_face, &subject.identity)
                .ok()
        })
        .collect();
    let property_pages = property.as_ref().map_or(0, |(_, _, pages)| *pages);
    let native_pages = if property_pages > 0
        && crate::property_mask::has_no_additional_native_primary_content_except(
            &property_face,
            &summarized,
        ) {
        0
    } else {
        scene.pages()
    };
    let pages = shared_pages + charts.len() + native_pages + property_pages;
    let page = page_tick % pages;
    if let Some(shared) = shared
        && page == 0
    {
        return Ok((shared.scene, page, pages, 100, true));
    }
    let chart_page = page - shared_pages;
    if chart_page < charts.len() {
        let show = charts[chart_page]
            .native_show_in_unit_at_size(
                preferred.unwrap_or(conduit_core::Unit::Celsius),
                width,
                height.saturating_sub(STATUS_STRIP_HEIGHT as u16),
            )
            .map_err(|e| format!("history graph refused: {e:?}"))?;
        return Ok((show.scene, page, pages, 100, true));
    }
    let content_page = chart_page - charts.len();
    if content_page < property_pages {
        let (graphics, _, _) = crate::property_mask::frame_with_context(
            &property_face,
            width,
            height,
            content_page,
            preferred,
            look,
            retained_snapshot,
        )
        .ok_or("property page refused")?;
        return Ok((graphics, page, pages, 100, false));
    }
    for _ in 0..content_page - property_pages {
        scene.turn_page(true);
    }
    let frame = scene
        .frame()
        .map_err(|e| format!("Face frame refused: {e:?}"))?;
    Ok((frame.scene, page, pages, scene.text_scale_percent(), false))
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod presentation_contract_tests;
