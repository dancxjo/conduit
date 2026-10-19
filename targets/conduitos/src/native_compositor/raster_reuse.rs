//! Bounded exact command reuse for a successfully retained surface.
//!
//! A changed command invalidates both its old and new footprint. Intersecting
//! non-fill commands extend that region until their entire original raster can
//! be replayed. Opaque rectangular fills may be clipped without changing their
//! geometry. All replay remains in original scene order, including text blending.
use super::surface_buffer::SurfaceBuffer;
use crate::display::{DisplayError, DisplayReceipt, PixelTarget};
use conduit_face::{
    GraphicsCommand, GraphicsCommandKind, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};

fn intersection(a: LayoutRect, b: LayoutRect) -> Option<LayoutRect> {
    let left = i32::from(a.x).max(i32::from(b.x));
    let top = i32::from(a.y).max(i32::from(b.y));
    let right = (i32::from(a.x) + i32::from(a.width)).min(i32::from(b.x) + i32::from(b.width));
    let bottom = (i32::from(a.y) + i32::from(a.height)).min(i32::from(b.y) + i32::from(b.height));
    (left < right && top < bottom).then_some(LayoutRect {
        x: left as i16,
        y: top as i16,
        width: (right - left).max(0) as u16,
        height: (bottom - top).max(0) as u16,
    })
}
fn union(a: LayoutRect, b: LayoutRect) -> LayoutRect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = (i32::from(a.x) + i32::from(a.width)).max(i32::from(b.x) + i32::from(b.width));
    let bottom = (i32::from(a.y) + i32::from(a.height)).max(i32::from(b.y) + i32::from(b.height));
    LayoutRect {
        x,
        y,
        width: (right - i32::from(x)) as u16,
        height: (bottom - i32::from(y)) as u16,
    }
}
fn fill(command: &GraphicsCommand) -> bool {
    command.kind == GraphicsCommandKind::Rect && command.style == GraphicsShapeStyle::Fill
}
fn footprint(command: &GraphicsCommand, extent: LayoutRect) -> Option<LayoutRect> {
    intersection(command.bounds, command.clip).and_then(|rect| intersection(rect, extent))
}
/// Public command fields can be edited after construction. Only canonical
/// scenes enter reuse; other inputs retain the original renderer and refusals.
pub(super) fn canonical(scene: &GraphicsScene) -> bool {
    let encoded = scene.encode();
    GraphicsScene::decode(&encoded[..scene.encoded_len()]).as_ref() == Ok(scene)
}

pub(super) fn raster(
    buffer: &mut SurfaceBuffer,
    scene: &GraphicsScene,
    previous: Option<&GraphicsScene>,
    render: impl FnOnce(&mut SurfaceBuffer, &GraphicsScene) -> Result<DisplayReceipt, DisplayError>,
) -> Result<DisplayReceipt, DisplayError> {
    let Some(previous) = previous else {
        buffer.clear();
        return render(buffer, scene);
    };
    let format = buffer.format();
    // Scene geometry has signed origins and canonical right/bottom limits.
    let extent = LayoutRect {
        x: 0,
        y: 0,
        width: format.width.min(i16::MAX as u32) as u16,
        height: format.height.min(i16::MAX as u32) as u16,
    };
    let mut dirty = None;
    let old = previous.commands();
    let new = scene.commands();
    for index in 0..old.len().max(new.len()) {
        if old.get(index) == new.get(index) {
            continue;
        }
        for command in [old.get(index), new.get(index)].into_iter().flatten() {
            if let Some(rect) = footprint(command, extent) {
                dirty = Some(dirty.map_or(rect, |prior| union(prior, rect)));
            }
        }
    }
    let Some(mut dirty) = dirty else {
        return Ok(DisplayReceipt::default());
    };
    // At most 48 distinct command footprints can extend the union. This also
    // handles command removal, reordering and partially overlapping glyphs.
    for _ in 0..=conduit_face::MAX_GRAPHICS_COMMANDS {
        let before = dirty;
        for command in new.iter().filter(|command| !fill(command)) {
            if let Some(rect) = footprint(command, extent)
                && intersection(rect, dirty).is_some()
            {
                dirty = union(dirty, rect);
            }
        }
        if before == dirty {
            break;
        }
    }
    let mut replay = GraphicsScene::empty();
    for command in new {
        let Some(rect) = footprint(command, extent).and_then(|rect| intersection(rect, dirty))
        else {
            continue;
        };
        let mut command = *command;
        if fill(&command) {
            command.clip = rect;
        }
        replay
            .push(command)
            .map_err(|_| DisplayError::InvalidExtent)?;
    }
    for y in u32::from(dirty.y as u16)..u32::from(dirty.y as u16) + u32::from(dirty.height) {
        let start = y as usize * format.width as usize + dirty.x as usize;
        buffer.pixels[start..start + usize::from(dirty.width)].fill(0);
    }
    render(buffer, &replay)
}

#[cfg(test)]
mod tests;
