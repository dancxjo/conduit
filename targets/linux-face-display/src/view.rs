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

pub struct Presentation<'a> {
    pub unit: Option<conduit_core::Unit>,
    pub look: Option<&'a conduit_face::LookInfo>,
    pub details: bool,
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
    } = presentation;
    if pixels.len() != usize::from(width) * usize::from(height) {
        return Err("raster extent mismatch".into());
    }
    let (frame, page, pages, scale, chart_viewport) =
        frame_with_look(face, width, height, page_tick, preferred, details, look)?;
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
    let property_face = face.clone();
    let property =
        crate::property_mask::frame_with_look(&property_face, width, height, 0, preferred, look);
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
        NativeFaceScene::prepare_visual_fallback(face, width, height, false, &admitted.visual)
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
    scene
        .set_local_notice(if unsupported {
            "Font missing characters; original text retained"
        } else {
            "Read only; Details for actions and facts"
        })
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
        && crate::property_mask::has_no_additional_native_primary_content(&property_face)
    {
        0
    } else {
        scene.pages()
    };
    let pages = charts.len() + native_pages + property_pages;
    let page = page_tick % pages;
    if page < charts.len() {
        let show = charts[page]
            .native_show_in_unit_at_size(
                preferred.unwrap_or(conduit_core::Unit::Celsius),
                width,
                height.saturating_sub(STATUS_STRIP_HEIGHT as u16),
            )
            .map_err(|e| format!("history graph refused: {e:?}"))?;
        return Ok((show.scene, page, pages, 100, true));
    }
    let content_page = page - charts.len();
    if content_page < property_pages {
        let (graphics, _, _) = crate::property_mask::frame_with_look(
            &property_face,
            width,
            height,
            content_page,
            preferred,
            look,
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
mod tests {
    use super::*;
    #[test]
    fn complete_info_property_pages_do_not_alternate_with_an_empty_snapshot() {
        let base = unavailable("unused");
        let make = |properties| {
            Face::new(
                9,
                base.basis.clone(),
                vec![FaceSubject {
                    identity: "instrument".into(),
                    role: FaceRole::Info,
                    name: "Room conditions".into(),
                }],
                vec![],
                properties,
                vec![],
            )
            .unwrap()
        };
        let property = conduit_face::FaceProperty {
            subject: "instrument".into(),
            name: "reading".into(),
            value: conduit_face::FacePropertyValue::Text("Real value".into()),
        };
        let face = make(vec![property.clone()]);
        for tick in 0..6 {
            let (scene, page, pages, _, _) = frame(face.clone(), 1024, 600, tick, None).unwrap();
            assert_eq!((page, pages), (0, 1));
            assert!(
                scene
                    .commands()
                    .iter()
                    .any(|c| c.payload() == "reading: Real value")
            );
        }
        let unsupported = conduit_face::FaceProperty {
            subject: "instrument".into(),
            name: "count".into(),
            value: conduit_face::FacePropertyValue::Count(12),
        };
        let mixed = make(vec![property, unsupported]);
        let (_, _, pages, _, _) = frame(mixed, 1024, 600, 0, None).unwrap();
        assert_eq!(pages, 1, "Count metadata adds no native primary content");
    }
    #[test]
    #[ignore = "requires explicitly selected private actual current Face"]
    fn actual_info_face_with_metadata_has_no_empty_page_in_any_cycle() {
        let path =
            std::env::var_os("CONDUIT_PRIVATE_CURRENT_FACE_TEST").expect("private actual Face");
        let document: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        let face: Face = serde_json::from_value(document["face"].clone()).unwrap();
        face.validate().unwrap();
        assert!(
            face.properties
                .iter()
                .any(|p| matches!(p.value, conduit_face::FacePropertyValue::Count(_)))
        );
        for tick in 0..8 {
            let (scene, page, pages, _, _) = frame(
                face.clone(),
                1024,
                600,
                tick,
                Some(conduit_core::Unit::Fahrenheit),
            )
            .unwrap();
            assert_eq!((page, pages), (0, 1));
            assert!(scene.commands().iter().any(|c| c.payload().contains("°F")));
            assert!(scene.commands().iter().any(|c| c.payload().contains('%')));
        }
    }
    #[test]
    fn sensor_pages_do_not_hide_transcript_from_the_same_face() {
        let base = unavailable("Transcript: the greenhouse is quiet");
        let face = Face::new(
            9,
            base.basis,
            vec![
                base.subjects[0].clone(),
                FaceSubject {
                    identity: "sensor".into(),
                    role: FaceRole::Info,
                    name: "Greenhouse".into(),
                },
            ],
            vec![],
            vec![conduit_face::FaceProperty {
                subject: "sensor".into(),
                name: "reading".into(),
                value: conduit_face::FacePropertyValue::Text("Sensor marker".into()),
            }],
            base.text,
        )
        .unwrap();
        let original = face.clone();
        let (_, _, pages, _, _) = frame(face.clone(), 1024, 600, 0, None).unwrap();
        assert!(pages >= 2);
        let mut sensor = false;
        let mut transcript = false;
        for tick in 0..pages {
            let (scene, page, count, _, _) = frame(face.clone(), 1024, 600, tick, None).unwrap();
            assert_eq!((page, count), (tick, pages));
            sensor |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("Sensor marker"));
            transcript |= scene
                .commands()
                .iter()
                .any(|c| c.payload().contains("greenhouse is quiet"));
        }
        assert!(
            sensor && transcript,
            "both contributions must remain perceptible"
        );
        assert_eq!(face, original);
    }
}

#[cfg(test)]
mod native_chart_pixel_tests {
    use super::*;
    use conduitos::display::{PixelTarget, RetainedPixelTarget};
    #[test]
    fn chart_pixel_is_written_once_without_stretching_and_preserves_status_strip() {
        let mut pixels = vec![0; 1024 * 600];
        let mut target = ChartViewport {
            width: 1024,
            height: 600,
            pixels: &mut pixels,
        };
        assert_eq!(target.format().width, 1024);
        assert_eq!(target.format().height, 564);
        target.write_pixel(511, 100, 0xabcdef).unwrap();
        assert_eq!(target.read_pixel(511, 100).unwrap(), 0xabcdef);
        assert!(target.write_pixel(1024, 100, 1).is_err());
        assert_eq!(pixels.iter().filter(|p| **p != 0).count(), 1);
        assert_eq!(pixels[(100 + 36) * 1024 + 511], 0xabcdef);
        assert!(pixels[..36 * 1024].iter().all(|p| *p == 0));
    }
}
