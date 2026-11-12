//! Generic bounded measurement-series manifestation in one ordinary document row.
//! No history, observation cache, clock alignment, Face mutation or action lives here.
use super::*;
use alloc::format;
use conduit_data::{
    MEASUREMENT_PLOT_FACE_KIND, MEASUREMENT_PLOT_SERIES_INFO_ID, MeasurementPlotSeries,
};
use conduit_face::{FaceProperty, FacePropertyValue, FaceRole, GraphicsPoint, GraphicsPolyline};

const MAX_SERIES: usize = 4;
const MAX_LABEL_BYTES: usize = 64;
// Four 32-point series need at most 3 continuous polylines each; endpoint
// markers, ticks, axes and all labels are charged before page admission.
pub(super) const COMMANDS: usize = 29;
const PAINTS: [GraphicsPaintRole; MAX_SERIES] = [
    GraphicsPaintRole::Accent,
    GraphicsPaintRole::Success,
    GraphicsPaintRole::Warning,
    GraphicsPaintRole::Focus,
];
struct Series<'a> {
    name: &'a str,
    data: MeasurementPlotSeries,
}
struct Chart<'a> {
    series: Vec<Series<'a>>,
    maximum_index: u64,
}

pub(super) fn is_series(property: &FaceProperty) -> bool {
    matches!(&property.value, FacePropertyValue::TypedValue { contract, .. }
        if contract.value_kind.as_str() == MEASUREMENT_PLOT_SERIES_INFO_ID)
}
fn chart(face: &Face, subject: usize) -> Result<Option<Chart<'_>>, FaceSceneError> {
    let subject = face
        .subjects
        .get(subject)
        .ok_or(FaceSceneError::InvalidFace)?;
    if subject.role != FaceRole::Semantic(conduit_core::kind_id(MEASUREMENT_PLOT_FACE_KIND)) {
        return Ok(None);
    }
    let mut series: Vec<Series<'_>> = Vec::with_capacity(MAX_SERIES);
    for property in face
        .properties
        .iter()
        .filter(|p| p.subject == subject.identity && is_series(p))
    {
        if series.len() == MAX_SERIES
            || property.name.is_empty()
            || property.name.len() > MAX_LABEL_BYTES
            || property.name.chars().any(char::is_control)
            || series.iter().any(|prior| prior.name == property.name)
        {
            return Err(FaceSceneError::UnsupportedStructure);
        }
        let FacePropertyValue::TypedValue { contract, bytes } = &property.value else {
            unreachable!()
        };
        if contract.maximum_bytes == 0
            || contract.maximum_bytes as usize > conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
            || bytes.len() > conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
            || contract.validate(bytes).is_err()
        {
            return Err(FaceSceneError::InvalidFace);
        }
        let data = conduit_data::decode_measurement_plot_series(bytes)
            .map_err(|_| FaceSceneError::InvalidFace)?;
        series.push(Series {
            name: &property.name,
            data,
        });
    }
    if series.is_empty() {
        return Err(FaceSceneError::UnsupportedStructure);
    }
    let maximum_index = series
        .iter()
        .map(|s| s.data.source_samples().saturating_sub(1) as u64)
        .max()
        .unwrap_or(0);
    Ok(Some(Chart {
        series,
        maximum_index,
    }))
}
impl Chart<'_> {
    fn labels(&self) -> Vec<String> {
        let mut labels = Vec::with_capacity(self.series.len() + 2);
        labels.push("Normalized value: 0–1".into());
        labels.push(format!("Window sample index: 0–{}", self.maximum_index));
        for series in &self.series {
            labels.push(format!(
                "{} · {} of {} points · {} omitted",
                series.name,
                series.data.points().len(),
                series.data.source_samples(),
                series.data.omitted_samples()
            ));
        }
        labels
    }
}

/// Decoding allocates bounded semantic intermediates (at most 4×32 points), in
/// Mask preparation/frame construction, never in an allocation-sealed Step.
pub(super) fn item(face: &Face, subject: usize) -> Result<Option<document::Item>, FaceSceneError> {
    let Some(chart) = chart(face, subject)? else {
        return Ok(None);
    };
    let mut item = document::item(chart.labels().join("\n"), GraphicsTextRole::Body);
    item.part = layout::NativePart::MeasurementChart(subject);
    Ok(Some(item))
}
pub(super) fn subject(part: layout::NativePart) -> Option<usize> {
    match part {
        layout::NativePart::MeasurementChart(subject) => Some(subject),
        _ => None,
    }
}
fn label_height(
    text: &str,
    width: u16,
    scale: u16,
    typography: Typography,
) -> Result<u16, FaceSceneError> {
    if text.len() > conduit_face::MAX_GRAPHICS_TEXT_BYTES {
        return Err(FaceSceneError::DocumentBound);
    }
    let height = TextLayout::new_selected(
        text,
        typography.style(GraphicsTextRole::Body.into()),
        width,
        scale,
    )
    .map_err(|_| FaceSceneError::InvalidExtent)?
    .finish_height();
    u16::try_from(height).map_err(|_| FaceSceneError::InvalidExtent)
}
fn plot_height(scale: u16, tick: u16) -> u16 {
    ((u32::from(scale) * 80 / 100)
        .max(64)
        .max(u32::from(tick) * 2 + 4)) as u16
}
pub(super) fn measure(
    text: &str,
    width: u16,
    maximum: u16,
    scale: u16,
    typography: Typography,
) -> Result<(usize, u16), FaceSceneError> {
    if width < 128 {
        return Err(FaceSceneError::InvalidExtent);
    }
    let tick = label_height("0", width, scale, typography)?;
    let mut height = plot_height(scale, tick)
        .checked_add(tick)
        .ok_or(FaceSceneError::InvalidExtent)?;
    for label in text.lines() {
        height = height
            .checked_add(label_height(label, width, scale, typography)?)
            .ok_or(FaceSceneError::InvalidExtent)?;
    }
    if height > maximum {
        return Err(FaceSceneError::InvalidExtent);
    }
    Ok((text.len(), height))
}
fn push(
    scene: &mut GraphicsScene,
    command: Result<GraphicsCommand, conduit_face::GraphicsError>,
) -> Result<(), FaceSceneError> {
    scene
        .push(command.map_err(|_| FaceSceneError::SceneBound)?)
        .map_err(|_| FaceSceneError::SceneBound)
}
fn text(
    scene: &mut GraphicsScene,
    bounds: LayoutRect,
    clip: LayoutRect,
    paint: GraphicsPaintRole,
    label: &str,
) -> Result<(), FaceSceneError> {
    push(
        scene,
        GraphicsCommand::text(bounds, clip, paint, label)
            .and_then(|c| c.with_text_role(GraphicsTextRole::Body)),
    )
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    face: &Face,
    subject: usize,
    bounds: LayoutRect,
    clip: LayoutRect,
    scale: u16,
    typography: Typography,
) -> Result<(), FaceSceneError> {
    let chart = chart(face, subject)?.ok_or(FaceSceneError::InvalidFace)?;
    let labels = chart.labels();
    let (_, required) = measure(
        &labels.join("\n"),
        bounds.width,
        bounds.height,
        scale,
        typography,
    )?;
    if required > bounds.height {
        return Err(FaceSceneError::InvalidExtent);
    }
    let heading = label_height(&labels[0], bounds.width, scale, typography)?;
    text(
        scene,
        LayoutRect {
            height: heading,
            ..bounds
        },
        clip,
        GraphicsPaintRole::Muted,
        &labels[0],
    )?;
    let tick = label_height("0", bounds.width, scale, typography)?;
    let left = ((u32::from(scale) * 24 / 100).max(24)) as u16;
    let coordinate = |origin: i16, offset: u16| {
        i16::try_from(i32::from(origin) + i32::from(offset))
            .map_err(|_| FaceSceneError::InvalidExtent)
    };
    let plot = LayoutRect {
        x: coordinate(bounds.x, left)?,
        y: coordinate(bounds.y, heading)?,
        width: bounds
            .width
            .checked_sub(left)
            .filter(|v| *v > 1)
            .ok_or(FaceSceneError::InvalidExtent)?,
        height: plot_height(scale, tick),
    };
    let _right = coordinate(plot.x, plot.width - 1)?;
    let bottom = coordinate(plot.y, plot.height - 1)?;
    coordinate(bounds.y, required)?;
    for rect in [
        LayoutRect { width: 1, ..plot },
        LayoutRect {
            y: bottom,
            height: 1,
            ..plot
        },
        LayoutRect {
            y: plot.y + (plot.height / 2) as i16,
            height: 1,
            ..plot
        },
    ] {
        push(
            scene,
            GraphicsCommand::rect(
                rect,
                clip,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    for (label, x, y) in [
        ("1", bounds.x, plot.y),
        ("0", bounds.x, bottom - tick as i16 + 1),
        ("0", plot.x, bottom + 1),
    ] {
        text(
            scene,
            LayoutRect {
                x,
                y,
                width: left,
                height: tick,
            },
            clip,
            GraphicsPaintRole::Muted,
            label,
        )?;
    }
    for (index, series) in chart.series.iter().enumerate() {
        let paint = PAINTS[index];
        let mut points = [GraphicsPoint::default(); conduit_face::MAX_GRAPHICS_POLYLINE_POINTS];
        let mut count = 0usize;
        let mut last = None;
        for point in series.data.points().iter() {
            let x = if chart.maximum_index == 0 {
                0
            } else {
                u128::from(*point.source_index()) * u128::from(plot.width - 1)
                    / u128::from(chart.maximum_index)
            } as i16;
            let y =
                (*point.value_millionths() * i64::from(plot.height - 1) / 1_000_000) as i16;
            let next = GraphicsPoint::new(plot.x + x, bottom - y)
                .map_err(|_| FaceSceneError::SceneBound)?;
            if last == Some(next) {
                continue;
            } // Coincident pixels, not omitted source points.
            if count == points.len() {
                push(
                    scene,
                    GraphicsCommand::polyline(
                        GraphicsPolyline::new(&points[..count])
                            .map_err(|_| FaceSceneError::SceneBound)?,
                        clip,
                        paint,
                    ),
                )?;
                points[0] = points[count - 1];
                count = 1;
            }
            points[count] = next;
            count += 1;
            last = Some(next);
        }
        if count > 1 {
            push(
                scene,
                GraphicsCommand::polyline(
                    GraphicsPolyline::new(&points[..count])
                        .map_err(|_| FaceSceneError::SceneBound)?,
                    clip,
                    paint,
                ),
            )?;
        }
        if let Some(last) = last {
            push(
                scene,
                GraphicsCommand::rect(
                    LayoutRect {
                        x: *last.x(),
                        y: *last.y(),
                        width: 1,
                        height: 1,
                    },
                    clip,
                    paint,
                    GraphicsShapeStyle::Fill,
                ),
            )?;
        }
    }
    let mut y = bottom + 1 + tick as i16;
    for (index, label) in labels.iter().enumerate().skip(1) {
        let height = label_height(label, bounds.width, scale, typography)?;
        let paint = if index == 1 {
            GraphicsPaintRole::Muted
        } else {
            PAINTS[index - 2]
        };
        text(
            scene,
            LayoutRect {
                y,
                height,
                ..bounds
            },
            clip,
            paint,
            label,
        )?;
        y += height as i16;
    }
    Ok(())
}

#[cfg(test)]
#[path = "measurement_chart/tests.rs"]
mod tests;
