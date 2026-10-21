//! Pointer inspection reuses the exact displayed cards, ports and Cord paths.
use super::*;

pub(super) fn cord_path(
    from: LayoutRect,
    to: LayoutRect,
    source_row: usize,
    sink_row: usize,
) -> Result<GraphicsPath, FaceSceneError> {
    let (start_x, end_x) = if from.x <= to.x {
        (from.x + from.width as i16 - 1, to.x)
    } else {
        (from.x, to.x + to.width as i16 - 1)
    };
    let start_y = from.y + 80 + source_row as i16 * PORT_ROW as i16;
    let end_y = to.y + 80 + sink_row as i16 * PORT_ROW as i16;
    let middle = if from.x == to.x {
        from.x + from.width as i16 + GAP as i16 / 2
    } else {
        start_x + (end_x - start_x) / 2
    };
    let mut points = Vec::with_capacity(4);
    points.push(GraphicsPoint::new(start_x, start_y).map_err(|_| FaceSceneError::SceneBound)?);
    if start_y != end_y {
        points.push(GraphicsPoint::new(middle, start_y).map_err(|_| FaceSceneError::SceneBound)?);
        points.push(GraphicsPoint::new(middle, end_y).map_err(|_| FaceSceneError::SceneBound)?);
    }
    points.push(GraphicsPoint::new(end_x, end_y).map_err(|_| FaceSceneError::SceneBound)?);
    GraphicsPath::new(&points).map_err(|_| FaceSceneError::SceneBound)
}

pub(in crate::native_face_scene) fn hit_test(
    face: &Face,
    screen: LayoutRect,
    page: usize,
    x: i16,
    y: i16,
) -> Result<Option<&str>, FaceSceneError> {
    let count = face
        .subjects
        .iter()
        .filter(|s| s.role == FaceRole::Gear)
        .count();
    let geometry = DiagramGeometry::new(screen, count);
    let ordering = super::super::connected_layout::order(face);
    let gears = visible_gears(
        face,
        &ordering,
        page * geometry.capacity(),
        geometry.capacity(),
    );
    for (index, gear) in gears.iter().enumerate() {
        let card = geometry.card(index);
        for (row, port) in ports(face, gear)
            .take(visible_ports(geometry.height, ports(face, gear).count()))
            .enumerate()
        {
            let bounds = LayoutRect {
                x: card.x + 12,
                y: card.y + 70 + row as i16 * PORT_ROW as i16,
                width: card.width - 24,
                height: 23,
            };
            if contains(bounds, x, y) {
                return Ok(Some(port.identity.as_str()));
            }
        }
        if contains(card, x, y) {
            return Ok(Some(gear.identity.as_str()));
        }
    }
    let mut shown = 0;
    let mut selected = None;
    for cord in face.subjects.iter().filter(|s| s.role == FaceRole::Cord) {
        let Some(((from, source_row), (to, sink_row))) =
            endpoints(face, &gears, cord, geometry.height)
        else {
            continue;
        };
        if shown == MAX_VISIBLE_LINKS {
            break;
        }
        shown += 1;
        let path = cord_path(geometry.card(from), geometry.card(to), source_row, sink_row)?;
        for pair in path.points().windows(2) {
            let (ax, ay, bx, by) = (*pair[0].x(), *pair[0].y(), *pair[1].x(), *pair[1].y());
            let near = if ax == bx {
                (i32::from(x) - i32::from(ax)).abs() <= 3 && y >= ay.min(by) && y <= ay.max(by)
            } else {
                (i32::from(y) - i32::from(ay)).abs() <= 3 && x >= ax.min(bx) && x <= ax.max(bx)
            };
            if near {
                // Later paths are drawn on top, so the visible Cord wins.
                selected = Some(cord.identity.as_str());
                break;
            }
        }
    }
    Ok(selected)
}
fn contains(b: LayoutRect, x: i16, y: i16) -> bool {
    i32::from(x) >= i32::from(b.x)
        && i32::from(y) >= i32::from(b.y)
        && i32::from(x) < i32::from(b.x) + i32::from(b.width)
        && i32::from(y) < i32::from(b.y) + i32::from(b.height)
}
