//! Appearance over explicit checked recipe groups; never domain-name grouping.
use super::*;

#[derive(Clone, Copy)]
struct Surface {
    group: u8,
    bounds: LayoutRect,
    marker: bool,
    existing_panel: bool,
}

/// Replace each group's budgeted divider with one cohesive surface. One pass
/// over the bounded document retains at most one slot per admitted page row.
/// Paint before rows so geometry order cannot occlude source reading order.
pub(in crate::native_face_scene) fn append_region_surfaces(
    scene: &mut GraphicsScene,
    rows: &[Row],
    page: usize,
    screen: LayoutRect,
    radius: Option<u8>,
) -> Result<(), FaceSceneError> {
    let mut surfaces: [Option<Surface>; MAX_PAGE_ROWS] = [None; MAX_PAGE_ROWS];
    let mut count = 0;
    for row in rows.iter().filter(|row| row.page == page) {
        let Some(group) = row.region else { continue };
        let bounds = padded_bounds(row)?;
        let index = if let Some(index) = surfaces[..count]
            .iter()
            .position(|slot| slot.is_some_and(|value| value.group == group))
        {
            index
        } else {
            if count == surfaces.len() {
                return Err(FaceSceneError::SceneBound);
            }
            surfaces[count] = Some(Surface {
                group,
                bounds,
                marker: false,
                existing_panel: false,
            });
            count += 1;
            count - 1
        };
        let surface = surfaces[index].as_mut().ok_or(FaceSceneError::SceneBound)?;
        surface.marker |= row.region_start;
        surface.existing_panel |= row.panel_bounds.is_some();
        let left = surface.bounds.x.min(bounds.x);
        let top = surface.bounds.y.min(bounds.y);
        let right = (i32::from(surface.bounds.x) + i32::from(surface.bounds.width))
            .max(i32::from(bounds.x) + i32::from(bounds.width));
        let bottom = (i32::from(surface.bounds.y) + i32::from(surface.bounds.height))
            .max(i32::from(bounds.y) + i32::from(bounds.height));
        surface.bounds = LayoutRect {
            x: left,
            y: top,
            width: u16::try_from(right - i32::from(left))
                .map_err(|_| FaceSceneError::InvalidExtent)?,
            height: u16::try_from(bottom - i32::from(top))
                .map_err(|_| FaceSceneError::InvalidExtent)?,
        };
    }
    for surface in surfaces[..count].iter().flatten() {
        // The earlier configured-panel path already owns its surface pair.
        if surface.marker && !surface.existing_panel {
            push(
                scene,
                GraphicsCommand::rect(
                    surface.bounds,
                    screen,
                    GraphicsPaintRole::Surface,
                    GraphicsShapeStyle::RoundedFill,
                )
                .and_then(|command| corners::apply(command, radius)),
            )?;
        }
    }
    Ok(())
}

fn padded_bounds(row: &Row) -> Result<LayoutRect, FaceSceneError> {
    let inset = i32::from(row.region_inset);
    Ok(LayoutRect {
        x: i16::try_from(i32::from(row.bounds.x) - inset)
            .map_err(|_| FaceSceneError::InvalidExtent)?,
        y: i16::try_from(i32::from(row.bounds.y) - inset)
            .map_err(|_| FaceSceneError::InvalidExtent)?,
        width: u16::try_from(i32::from(row.bounds.width) + inset * 2)
            .map_err(|_| FaceSceneError::InvalidExtent)?,
        height: u16::try_from(i32::from(row.bounds.height) + inset * 2)
            .map_err(|_| FaceSceneError::InvalidExtent)?,
    })
}
