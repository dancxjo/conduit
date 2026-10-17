//! Paginated graphics rows measured by the same typography cursor as rasterization.
use super::*;
use alloc::string::String;

pub(super) const HEADER: u16 = 56;
pub(super) const FOOTER: u16 = 56;
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
}

impl Row {
    pub fn text_bounds(&self) -> LayoutRect {
        LayoutRect {
            x: self.bounds.x + self.padding as i16,
            y: self.bounds.y + self.padding as i16,
            width: self.bounds.width - self.padding * 2,
            height: self.bounds.height - self.padding * 2,
        }
    }
}

/// Row-local drawing belongs with its measured bounds. A heading marker and
/// control outline are Mask geometry; neither changes the Face or hit target.
pub(super) fn append_row_chrome(
    scene: &mut GraphicsScene,
    row: &Row,
    screen: LayoutRect,
    outline: GraphicsPaintRole,
    showing_details: bool,
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
            GraphicsCommand::rect(row.bounds, screen, outline, GraphicsShapeStyle::Stroke),
        )?;
    }
    if !showing_details
        && matches!(
            row.role,
            GraphicsTextRole::Title | GraphicsTextRole::Heading
        )
    {
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
    if !(1..=2).contains(&columns) || !(SPACE_SM..=SPACE_SM * 2).contains(&padding) {
        return Err(FaceSceneError::InvalidExtent);
    }
    let width = screen
        .width
        .checked_sub(gutter.checked_mul(2).ok_or(FaceSceneError::InvalidExtent)?)
        .and_then(|width| width.checked_sub(gutter * (columns - 1)))
        .map(|width| width / columns)
        .filter(|width| *width > 0)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let bottom = screen
        .height
        .checked_sub(FOOTER)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let maximum_height = bottom
        .checked_sub(HEADER)
        .and_then(|height| height.checked_sub(padding * 2))
        .filter(|height| *height > 0)
        .ok_or(FaceSceneError::InvalidExtent)?;
    let mut column = 0u16;
    let mut rows = Vec::new();
    let (mut page, mut count, mut y, mut bytes) = (0, 0, HEADER, 0usize);
    let mut previous_region = None;
    for item in items {
        let mut region_start = item.region.is_some() && item.region != previous_region;
        previous_region = item.region;
        let inset = u16::from(item.indent) * INDENT_STEP;
        let item_width = width
            .checked_sub(inset)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let text_width = item_width
            .checked_sub(padding * 2)
            .filter(|width| *width > 0)
            .ok_or(FaceSceneError::InvalidExtent)?;
        bytes = bytes
            .checked_add(item.text.len())
            .ok_or(FaceSceneError::DocumentBound)?;
        if bytes > MAX_DOCUMENT_BYTES {
            return Err(FaceSceneError::DocumentBound);
        }
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let mut end = remaining.len().min(conduit_face::MAX_GRAPHICS_TEXT_BYTES);
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            let height = loop {
                if end == 0 {
                    return Err(FaceSceneError::InvalidExtent);
                }
                let height = TextLayout::new_scaled(
                    &remaining[..end],
                    item.role.into(),
                    text_width,
                    text_scale_percent,
                )
                .map_err(|_| FaceSceneError::InvalidExtent)?
                .finish_height();
                if height <= u32::from(maximum_height) {
                    break height as u16;
                }
                end -= 1;
                while !remaining.is_char_boundary(end) {
                    end -= 1;
                }
            };
            let box_height = height + padding * 2;
            if y + box_height > bottom || count == MAX_PAGE_ROWS {
                column += 1;
                if column == columns || count == MAX_PAGE_ROWS {
                    page += 1;
                    count = 0;
                    column = 0;
                }
                y = HEADER;
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
            remaining = &remaining[end..];
        }
    }
    Ok(rows)
}
