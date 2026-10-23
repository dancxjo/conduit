//! Existing text chunking and exact control bindings, retained in bounded fragments.
use super::*;
pub(super) fn arrange(
    context: &Context<'_>,
    region: Option<usize>,
    width: u16,
    height: u16,
) -> Result<Vec<Fragment>, FaceSceneError> {
    let maximum_height = inset(height, context.padding)?;
    let mut fragments = Vec::new();
    let mut current = Fragment::empty();
    let mut marker = region.is_some();
    let mut items = context.items(region).peekable();
    let mut previous = None;
    let mut cluster_atomic = false;
    while let Some((original, item)) = items.next() {
        let starts = field_clusters::starts(previous, item);
        previous = field_clusters::key(item);
        if starts {
            let demand = field_clusters::measure(
                core::iter::once(item).chain(items.clone().map(|(_, item)| item)),
                field_clusters::Metrics {
                    width,
                    maximum_height,
                    scale: context.scale,
                    typography: context.typography,
                    padding: context.padding,
                },
                marker,
            )?;
            cluster_atomic = demand.is_some();
            if demand.is_some_and(|demand| {
                !demand.fits(
                    current.height
                        + if current.entries.is_empty() {
                            0
                        } else {
                            SPACE_SM
                        },
                    height,
                    current.entries.len(),
                    current.cost + chrome::COMMANDS + 1,
                )
            }) && !current.entries.is_empty()
            {
                fragments.push(current);
                current = Fragment::empty();
            }
        } else if previous.is_none() {
            cluster_atomic = false;
        }

        let item_inset = u16::from(item.indent)
            .checked_mul(INDENT_STEP)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let item_width = width
            .checked_sub(item_inset)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let text_width = field_clusters::text_width(item, item_width, context.padding)?;
        let mut remaining = item.text.as_str();
        let mut chunk = 0;
        while !remaining.is_empty() {
            if context.rows.get() == MAX_ROWS {
                return Err(FaceSceneError::DocumentBound);
            }
            let (end, measured) = measure_prefix(
                remaining,
                item.role,
                text_width,
                maximum_height,
                context.scale,
                context.typography,
            )?;
            let box_height = add(measured, context.padding * 2)?;
            let gap = if current.entries.is_empty() {
                0
            } else {
                SPACE_SM
            };
            let y = add(current.height, gap)?;
            let bottom = add(y, box_height)?;
            let cost = row_commands(item, marker);
            if !current.fits(bottom, 1, cost, height) {
                if current.entries.is_empty() {
                    return Err(FaceSceneError::SceneBound);
                }
                fragments.push(current);
                current = Fragment::empty();
            }
            let y = if current.entries.is_empty() {
                0
            } else {
                add(current.height, SPACE_SM)?
            };
            current.height = add(y, box_height)?;
            current.cost += cost;
            current.entries.push(Entry {
                original,
                chunk,
                cross_extent: 0,
                cost,
                break_before: !cluster_atomic || (starts && chunk == 0),
                outer_padding: 0,
                row: Row {
                    text: remaining[..end].into(),
                    role: item.role,
                    paint: item.paint,
                    control: item.control,
                    subject_index: item.subject_index,
                    page: 0,
                    bounds: LayoutRect {
                        x: item_inset as i16,
                        y: y as i16,
                        width: item_width,
                        height: box_height,
                    },
                    region_start: marker,
                    padding: context.padding,
                    checkbox: item.checkbox,
                    icon: item.icon,
                    panel_bounds: None,
                },
            });
            context.rows.set(context.rows.get() + 1);
            marker = false;
            chunk += 1;
            remaining = &remaining[end..];
        }
    }
    if !current.entries.is_empty() {
        fragments.push(current);
    }
    Ok(fragments)
}
