//! Paginated graphics rows measured by the same typography cursor as rasterization.
use super::*;
use crate::display::typography::Typography;
use alloc::string::String;

#[path = "layout/configured.rs"]
mod configured;
#[cfg(test)]
pub(super) use configured::admit_configured;
pub(super) use configured::{ConfiguredColumns, admit_configured_selected, authored_panel_groups};

#[path = "layout/field_clusters.rs"]
mod field_clusters;
#[path = "layout/panels.rs"]
mod panels;

pub(super) use super::chrome::{FOOTER, HEADER};
pub(super) const MAX_PAGE_ROWS: usize = 20;
const MAX_ROWS: usize = 32_768;
const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;
const INDENT_STEP: u16 = 14;

pub(super) struct Row {
    pub text: String,
    pub role: GraphicsTextRole,
    pub paint: GraphicsPaintRole,
    pub control: Option<FaceControl>,
    pub subject_index: Option<usize>,
    pub page: usize,
    pub bounds: LayoutRect,
    pub region_start: bool,
    pub padding: u16,
    pub checkbox: Option<bool>,
    pub icon: Option<conduit_face::FaceIconKey>,
    pub panel_bounds: Option<LayoutRect>,
}

impl Row {
    /// Source-owned primary disclosure becomes a filled action only while available.
    pub fn emphasized(&self) -> bool {
        self.role == GraphicsTextRole::Heading
            && self.paint == GraphicsPaintRole::Accent
            && self
                .control
                .is_some_and(|control| control.argument.is_none())
    }

    pub fn text_paint(&self) -> GraphicsPaintRole {
        if self.emphasized() {
            GraphicsPaintRole::Background
        } else {
            self.paint
        }
    }

    pub fn text_bounds(&self) -> LayoutRect {
        let inset = if self.checkbox.is_some() {
            super::boolean_choice::INSET
        } else {
            0
        } + super::semantic_icons::inset(self.icon);
        LayoutRect {
            x: self.bounds.x + self.padding as i16 + inset as i16,
            y: self.bounds.y + self.padding as i16,
            width: self.bounds.width - self.padding * 2 - inset,
            height: self.bounds.height - self.padding * 2,
        }
    }
}

/// Draw panel surfaces before any rows: authored geometry order may differ
/// from the source reading order restored after layout admission.
pub(super) fn append_panel_chrome(
    scene: &mut GraphicsScene,
    row: &Row,
    screen: LayoutRect,
) -> Result<(), FaceSceneError> {
    if let Some(bounds) = row.panel_bounds {
        for (paint, shape) in [
            (GraphicsPaintRole::Surface, GraphicsShapeStyle::RoundedFill),
            (GraphicsPaintRole::Border, GraphicsShapeStyle::RoundedStroke),
        ] {
            push(scene, GraphicsCommand::rect(bounds, screen, paint, shape))?;
        }
    }
    Ok(())
}

/// Row-local drawing belongs with its measured bounds. A heading marker and
/// control outline are Mask geometry; neither changes the Face or hit target.
pub(super) fn append_row_chrome(
    scene: &mut GraphicsScene,
    row: &Row,
    screen: LayoutRect,
    outline: GraphicsPaintRole,
    showing_details: bool,
    focused: bool,
) -> Result<(), FaceSceneError> {
    if !showing_details && row.region_start {
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: row.bounds.x,
                    y: row.bounds.y,
                    width: row.bounds.width,
                    height: 2,
                },
                screen,
                GraphicsPaintRole::Muted,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    if row.control.is_some() {
        push(
            scene,
            GraphicsCommand::rect(
                row.bounds,
                screen,
                if row.emphasized() {
                    GraphicsPaintRole::Accent
                } else if focused || row.paint == GraphicsPaintRole::Muted {
                    outline
                } else {
                    GraphicsPaintRole::Border
                },
                if row.emphasized() {
                    GraphicsShapeStyle::RoundedFill
                } else {
                    GraphicsShapeStyle::RoundedStroke
                },
            ),
        )?;
    }
    // A second inset outline distinguishes focus even with identical paints
    // (forced colors or monochrome scanout). It adds no semantic state.
    if focused {
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: row.bounds.x + 3,
                    y: row.bounds.y + 3,
                    width: row.bounds.width - 6,
                    height: row.bounds.height - 6,
                },
                screen,
                outline,
                GraphicsShapeStyle::Stroke,
            ),
        )?;
    }
    if !showing_details
        && matches!(
            row.role,
            GraphicsTextRole::Title | GraphicsTextRole::Heading
        )
    {
        if row.emphasized() {
            push(
                scene,
                GraphicsCommand::rect(
                    row.bounds,
                    screen,
                    if focused {
                        outline
                    } else {
                        GraphicsPaintRole::Border
                    },
                    GraphicsShapeStyle::RoundedStroke,
                ),
            )?;
            return Ok(());
        }
        push(
            scene,
            GraphicsCommand::rect(
                LayoutRect {
                    x: row.bounds.x,
                    y: row.bounds.y + 4,
                    width: 3,
                    height: row.bounds.height - 8,
                },
                screen,
                GraphicsPaintRole::Accent,
                GraphicsShapeStyle::Fill,
            ),
        )?;
    }
    Ok(())
}

pub(super) fn admit(
    items: Vec<document::Item>,
    screen: LayoutRect,
) -> Result<Vec<Row>, FaceSceneError> {
    admit_with_expression(items, screen, SPACE_XL, 1, 100, SPACE_SM)
}

/// Geometry belongs to this admitted Mask; semantic rows remain in their
/// existing order and retain exact action/argument references across columns.
pub(super) fn admit_with_expression(
    items: Vec<document::Item>,
    screen: LayoutRect,
    gutter: u16,
    columns: u16,
    text_scale_percent: u16,
    padding: u16,
) -> Result<Vec<Row>, FaceSceneError> {
    admit_with_expression_selected(
        items,
        screen,
        gutter,
        columns,
        text_scale_percent,
        padding,
        Typography::EDITORIAL,
    )
}

pub(super) fn admit_with_expression_selected(
    items: Vec<document::Item>,
    screen: LayoutRect,
    gutter: u16,
    columns: u16,
    text_scale_percent: u16,
    padding: u16,
    typography: Typography,
) -> Result<Vec<Row>, FaceSceneError> {
    field_clusters::validate(&items)?;
    if !(1..=2).contains(&columns) || !(SPACE_SM..=SPACE_SM * 2).contains(&padding) {
        return Err(FaceSceneError::InvalidExtent);
    }
    // A recipe permits columns; the narrow encounter admits one measured
    // reading column while retaining the original semantic/control order.
    let columns = if screen.width < 640 { 1 } else { columns };
    let width = screen
        .width
        .checked_sub(gutter.checked_mul(2).ok_or(FaceSceneError::InvalidExtent)?)
        .and_then(|width| width.checked_sub(gutter * (columns - 1)))
        .map(|width| width / columns)
        .filter(|width| *width > 0)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let chrome = chrome::Chrome::prepare_selected(screen, text_scale_percent, typography)?;
    let bottom = chrome.bottom;
    let padding = fit_padding(
        items.iter(),
        bottom - chrome.top,
        text_scale_percent,
        padding,
        typography,
    );
    let maximum_height = bottom
        .checked_sub(chrome.top)
        .and_then(|height| height.checked_sub(padding * 2))
        .filter(|height| *height > 0)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let mut column = 0u16;
    let mut rows = Vec::new();
    let (mut page, mut count, mut y, mut bytes) = (0, 0, chrome.top, 0usize);
    let mut previous_region = None;
    // Reserve one non-color focus marker per page, including continuation pages.
    let mut commands = chrome::COMMANDS + 1;
    let mut items = items.into_iter();
    let mut previous_item = None;
    while let Some(item) = items.next() {
        let cluster_start = field_clusters::starts(previous_item, &item);
        previous_item = field_clusters::key(&item);
        let mut region_start = item.region.is_some() && item.region != previous_region;
        previous_region = item.region;
        let inset = u16::from(item.indent) * INDENT_STEP;
        let item_width = width
            .checked_sub(inset)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let text_width = field_clusters::text_width(&item, item_width, padding)?;
        if cluster_start
            && let Some(demand) = field_clusters::measure(
                core::iter::once(&item).chain(items.as_slice().iter()),
                field_clusters::Metrics {
                    width,
                    maximum_height,
                    scale: text_scale_percent,
                    typography,
                    padding,
                },
                region_start,
            )?
            && demand.fits(chrome.top, bottom, 0, chrome::COMMANDS + 1)
            && !demand.fits(y, bottom, count, commands)
        {
            column += 1;
            if column == columns || !demand.fits(chrome.top, bottom, count, commands) {
                page += 1;
                count = 0;
                column = 0;
                commands = chrome::COMMANDS + 1;
            }
            y = chrome.top;
        }
        bytes = bytes
            .checked_add(item.text.len())
            .ok_or(FaceSceneError::DocumentBound)?;
        if bytes > MAX_DOCUMENT_BYTES {
            return Err(FaceSceneError::DocumentBound);
        }
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let (end, height) = measure_prefix(
                remaining,
                item.role,
                text_width,
                maximum_height,
                text_scale_percent,
                typography,
            )?;
            let box_height = height + padding * 2;
            let cost = row_commands(&item, region_start);
            let command_pressure = commands + cost > conduit_face::MAX_GRAPHICS_COMMANDS;
            if y + box_height > bottom || count == MAX_PAGE_ROWS || command_pressure {
                column += 1;
                if column == columns || count == MAX_PAGE_ROWS || command_pressure {
                    page += 1;
                    count = 0;
                    column = 0;
                    commands = chrome::COMMANDS + 1;
                }
                y = chrome.top;
            }
            if rows.len() == MAX_ROWS {
                return Err(FaceSceneError::DocumentBound);
            }
            rows.push(Row {
                text: remaining[..end].into(),
                role: item.role,
                paint: item.paint,
                control: item.control,
                subject_index: item.subject_index,
                page,
                region_start,
                padding,
                checkbox: item.checkbox,
                icon: item.icon,
                panel_bounds: None,
                bounds: LayoutRect {
                    x: (gutter + column * (width + gutter) + inset) as i16,
                    y: y as i16,
                    width: item_width,
                    height: box_height,
                },
            });
            region_start = false;
            y += box_height + SPACE_SM;
            count += 1;
            commands += cost;
            remaining = &remaining[end..];
        }
    }
    Ok(rows)
}

/// Density-derived spacing is preferred geometry, not an authored hard minimum.
/// At a constrained extent retain the requested text scale and the existing
/// admitted minimum padding. If even that cannot fit, ordinary admission refuses.
fn fit_padding<'a>(
    items: impl Iterator<Item = &'a document::Item>,
    available: u16,
    scale: u16,
    preferred: u16,
    typography: Typography,
) -> u16 {
    let needs_minimum = items.filter(|item| !item.text.is_empty()).any(|item| {
        TextLayout::new_selected("", typography.style(item.role.into()), 1, scale).is_ok_and(
            |layout| layout.finish_height() + u32::from(preferred) * 2 > u32::from(available),
        )
    });
    if needs_minimum { SPACE_SM } else { preferred }
}

fn row_commands(item: &document::Item, region_start: bool) -> usize {
    1 + usize::from(item.icon.is_some())
        + 2 * usize::from(item.checkbox.is_some())
        + usize::from(item.control.is_some())
        + usize::from(region_start)
        + usize::from(matches!(
            item.role,
            GraphicsTextRole::Title | GraphicsTextRole::Heading
        ))
}

fn measure_prefix(
    remaining: &str,
    role: GraphicsTextRole,
    width: u16,
    maximum_height: u16,
    scale: u16,
    typography: Typography,
) -> Result<(usize, u16), FaceSceneError> {
    let mut end = remaining.len().min(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
    while !remaining.is_char_boundary(end) {
        end -= 1;
    }
    loop {
        if end == 0 {
            return Err(FaceSceneError::InvalidExtent);
        }
        let height = TextLayout::new_selected(
            &remaining[..end],
            typography.style(role.into()),
            width,
            scale,
        )
        .map_err(|_| FaceSceneError::InvalidExtent)?
        .finish_height();
        if height <= u32::from(maximum_height) {
            return Ok((end, height as u16));
        }
        end -= 1;
        while !remaining.is_char_boundary(end) {
            end -= 1;
        }
    }
}

#[cfg(test)]
#[path = "layout/field_cluster_tests.rs"]
mod field_cluster_tests;
