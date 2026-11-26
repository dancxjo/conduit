//! Finite ordinary content plots over shared, immutable quantity-series facts.
use super::*;
use conduit_face::{PreparedQuantitySeries, ProjectedQuantitySeries, QUANTITY_SERIES_ROLE};

// Admission includes two axes and every retained point, even when fewer render.
pub(super) const COMMANDS: usize = 18;
const MAX_SERIES: usize = 64;

pub(super) fn prepare(
    face: &Face,
    items: &mut Vec<document::Item>,
    preferred: Option<conduit_core::Unit>,
    details: &mut Vec<document::Item>,
) -> Vec<ProjectedQuantitySeries> {
    let mut plots = Vec::new();
    for (subject_index, subject) in face.subjects.iter().enumerate() {
        if plots.len() == MAX_SERIES {
            break;
        }
        if !matches!(&subject.role, conduit_face::FaceRole::Semantic(role) if role.as_str() == QUANTITY_SERIES_ROLE)
        {
            continue;
        }
        let Ok(series) =
            PreparedQuantitySeries::prepare(face, &subject.identity, 64).and_then(|series| {
                let unit = preferred.filter(|unit| {
                    series
                        .samples()
                        .iter()
                        .find_map(|sample| sample.value)
                        .is_some_and(|value| value.dimension() == unit.dimension())
                });
                series
                    .project(unit, 16)
                    .map_err(|_| conduit_face::QuantitySeriesRefusal::Quantity)
            })
        else {
            continue;
        };
        if let Some(bytes) = &series.publication_reference {
            let mut reference = document::plot_item(subject_index, plots.len());
            reference.plot = None;
            reference.text = alloc::format!(
                "{} publication reference canonical bytes: {:?}; evidence: {}",
                subject.name,
                bytes,
                series
                    .publication_evidence
                    .as_deref()
                    .unwrap_or("unspecified")
            );
            details.push(reference);
        }
        if series.points.is_empty() {
            continue;
        }
        let Some(position) = items
            .iter()
            .position(|item| item.subject_index == Some(subject_index))
        else {
            continue;
        };
        let mut plot = document::plot_item(subject_index, plots.len());
        plot.region = items[position].region;
        plot.region_order = items[position].region_order;
        let mut caption = document::plot_item(subject_index, plots.len());
        caption.plot = None;
        caption.text = alloc::format!(
            "{} to {}; producer Boot {}–{} ms; {} plotted, {} omitted points, {} unavailable; {} gaps; {} outside view.",
            series.minimum.as_deref().unwrap_or("unavailable"),
            series.maximum.as_deref().unwrap_or("unavailable"),
            series.first_boot_ms.as_deref().unwrap_or("?"),
            series.last_boot_ms.as_deref().unwrap_or("?"),
            series.points.len(),
            series.omitted_points,
            series.unavailable_samples,
            series.gap_runs,
            series.omitted_samples,
        );
        items.insert(position + 1, caption);
        items.insert(position + 2, plot);
        plots.push(series);
    }
    plots
}

pub(super) fn append(
    scene: &mut GraphicsScene,
    series: &ProjectedQuantitySeries,
    bounds: LayoutRect,
    clip: LayoutRect,
) -> Result<(), FaceSceneError> {
    if bounds.width == 0 || bounds.height == 0 {
        return Err(FaceSceneError::InvalidExtent);
    }
    let horizontal = LayoutRect {
        y: bounds.y + (bounds.height - 1) as i16,
        height: 1,
        ..bounds
    };
    let vertical = LayoutRect { width: 1, ..bounds };
    for axis in [horizontal, vertical] {
        push(
            scene,
            GraphicsCommand::rect(
                axis,
                clip,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    // Independent points never imply continuity across a missing/unavailable sample
    // or an omitted point. Both axes use the shared exact scaled projection.
    for point in &series.points {
        let Some(value) = point.value_millionths else {
            continue;
        };
        let width = bounds.width.min(3);
        let height = bounds.height.min(3);
        let x = u64::from(point.time_millionths) * u64::from(bounds.width - width) / 1_000_000;
        let y = u64::from(1_000_000 - value) * u64::from(bounds.height - height) / 1_000_000;
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: bounds.x + x as i16,
                    y: bounds.y + y as i16,
                    width,
                    height,
                },
                clip,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "quantity_series_tests.rs"]
mod tests;
