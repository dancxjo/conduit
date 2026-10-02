//! Paginated graphics rows measured by the same typography cursor as rasterization.
use super::*;
use alloc::string::String;

pub(super) const FOOTER: u16 = 80;
pub(super) const MAX_PAGE_ROWS: usize = 20;
const MAX_ROWS: usize = 32_768;
const MAX_DOCUMENT_BYTES: usize = 2 * 1024 * 1024;

pub(super) struct Row {
    pub text: String,
    pub role: GraphicsTextRole,
    pub paint: GraphicsPaintRole,
    pub control: Option<FaceControl>,
    pub page: usize,
    pub bounds: LayoutRect,
}

impl Row {
    pub fn text_bounds(&self) -> LayoutRect {
        LayoutRect {
            x: self.bounds.x + SPACE_SM as i16,
            y: self.bounds.y + SPACE_SM as i16,
            width: self.bounds.width - SPACE_SM * 2,
            height: self.bounds.height - SPACE_SM * 2,
        }
    }
}

pub(super) fn admit(
    items: Vec<document::Item>,
    screen: LayoutRect,
) -> Result<Vec<Row>, FaceSceneError> {
    let width = screen.width - SPACE_XL * 2;
    let text_width = width - SPACE_SM * 2;
    let bottom = screen.height - FOOTER;
    let maximum_height = bottom - SPACE_XL - SPACE_SM * 2;
    let mut rows = Vec::new();
    let (mut page, mut count, mut y, mut bytes) = (0, 0, SPACE_XL, 0usize);
    for item in items {
        bytes = bytes
            .checked_add(item.text.len())
            .ok_or(FaceSceneError::DocumentBound)?;
        if bytes > MAX_DOCUMENT_BYTES {
            return Err(FaceSceneError::DocumentBound);
        }
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let mut end = remaining
                .len()
                .min(conduit_presentation::MAX_GRAPHICS_TEXT_BYTES);
            while !remaining.is_char_boundary(end) {
                end -= 1;
            }
            let height = loop {
                if end == 0 {
                    return Err(FaceSceneError::InvalidExtent);
                }
                let height = TextLayout::new(&remaining[..end], item.role.into(), text_width)
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
            let box_height = height + SPACE_SM * 2;
            if y + box_height > bottom || count == MAX_PAGE_ROWS {
                page += 1;
                count = 0;
                y = SPACE_XL;
            }
            if rows.len() == MAX_ROWS {
                return Err(FaceSceneError::DocumentBound);
            }
            rows.push(Row {
                text: remaining[..end].into(),
                role: item.role,
                paint: item.paint,
                control: item.control,
                page,
                bounds: LayoutRect {
                    x: SPACE_XL as i16,
                    y: y as i16,
                    width,
                    height: box_height,
                },
            });
            y += box_height + SPACE_SM;
            count += 1;
            remaining = &remaining[end..];
        }
    }
    Ok(rows)
}
