//! Finite adjacent lanes for the first authored pair of checked groups.
//! Original rows/actions never move in the document vector. Later groups and
//! unmatched reading span the available width; only geometry is assigned here.
use super::*;

#[path = "panels/ordering.rs"]
mod ordering;
const LEGACY_EDGE: u16 = SPACE_SM;

#[path = "panels/flow.rs"]
mod flow;
use flow::Flow;

pub(super) fn admit(
    items: Vec<document::Item>,
    gutter: u16,
    spacing: RowSpacing,
    groups: Option<[u8; 2]>,
    viewport: &Viewport,
) -> Result<Vec<Row>, FaceSceneError> {
    let scale = viewport.scale;
    let typography = viewport.typography;
    let spacing = spacing.validate()?;
    let padding = spacing.padding;
    let items = ordering::prepare(items, groups)?;
    // Empty authored regions do not reserve space. This decision is derived
    // from admitted rows, without changing source subjects or reading order.
    let paired_regions = groups.is_some_and(|groups| {
        groups.iter().all(|group| {
            items
                .iter()
                .any(|(_, item)| item.region == Some(*group) && !item.text.is_empty())
        })
    });
    let mut flow = Flow::new(viewport, gutter, groups, paired_regions, spacing)?;
    let padding = fit_padding(
        items.iter().map(|(_, item)| item),
        (flow.bottom - flow.top).saturating_sub(flow.edge * 2),
        scale,
        padding,
        typography,
    );
    let spacing = RowSpacing { padding, ..spacing };
    let mut rows = Vec::new();
    let mut reading_order = Vec::new();
    let mut bytes = 0usize;
    let mut previous_region = None;
    let mut items = items.into_iter();
    let mut previous_item = None;
    while let Some((original, item)) = items.next() {
        let insets = spacing.insets(&item);
        let cluster_start = field_clusters::starts(previous_item, &item);
        previous_item = field_clusters::key(&item);
        bytes = bytes
            .checked_add(item.text.len())
            .filter(|value| *value <= MAX_DOCUMENT_BYTES)
            .ok_or(FaceSceneError::DocumentBound)?;
        let lane = flow.lane(item.region);
        flow.select(lane, item.region, &mut rows);
        let inset = u16::from(item.indent) * INDENT_STEP;
        let available_height = flow
            .bottom
            .checked_sub(flow.top)
            .and_then(|value| {
                value.checked_sub(if lane.is_some() || flow.span.is_some() {
                    flow.edge * 2
                } else {
                    0
                })
            })
            .filter(|value| *value > 0)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let maximum_height = available_height
            .checked_sub(insets.block * 2)
            .filter(|height| *height > 0)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let mut region_start = item.region.is_some() && item.region != previous_region;
        previous_region = item.region;
        if cluster_start {
            let measure = |width| {
                field_clusters::measure(
                    core::iter::once(&item).chain(items.as_slice().iter().map(|(_, item)| item)),
                    field_clusters::Metrics {
                        width,
                        maximum_height: available_height,
                        scale,
                        typography,
                        spacing,
                    },
                    region_start,
                )
            };
            let edge = if lane.is_some() || flow.span.is_some() {
                flow.edge
            } else {
                0
            };
            let bottom = flow.bottom - edge;
            let current = match measure(flow.width(lane)) {
                Err(FaceSceneError::InvalidExtent)
                    if flow.continuation_width(item.region) > flow.width(lane) =>
                {
                    None
                }
                result => result?,
            };
            let current_fits = current.as_ref().is_some_and(|demand| {
                demand.fits(
                    flow.y(lane),
                    bottom,
                    flow.count,
                    flow.commands + 2 * usize::from(flow.new_panel(lane)),
                )
            });
            if !current_fits
                && let Some(demand) = measure(flow.continuation_width(item.region))?
                && demand.fits(
                    flow.top + edge,
                    bottom,
                    0,
                    chrome::COMMANDS + 1 + 2 * usize::from(lane.is_some()),
                )
            {
                let cause = current
                    .and_then(|demand| {
                        demand.pressure(
                            flow.y(lane),
                            bottom,
                            flow.count,
                            flow.commands + 2 * usize::from(flow.new_panel(lane)),
                        )
                    })
                    .unwrap_or_else(LayoutContinuation::measurement_refusal)
                    .keeping_group();
                flow.next_page(item.region, &mut rows, cause);
            }
        }

        let mut remaining = item.text.as_str();
        let mut chunk = 0usize;
        while !remaining.is_empty() {
            let mut advanced = false;
            let (lane, width, end, height, cost) = loop {
                let lane = flow.lane(item.region);
                let measured = (|| {
                    let width = flow
                        .width(lane)
                        .checked_sub(inset)
                        .ok_or(FaceSceneError::InvalidExtent)?;
                    let text_width = field_clusters::text_width(&item, width, insets, scale)?;
                    let (end, height) = measure_prefix(
                        remaining,
                        item.role,
                        text_width,
                        maximum_height,
                        scale,
                        typography,
                    )?;
                    Ok((width, end, height))
                })();
                let (width, end, height) = match measured {
                    // An indivisible glyph/indent may not fit the narrow lane.
                    // Retry only when continuation actually provides more width.
                    Err(FaceSceneError::InvalidExtent)
                        if !advanced && flow.continuation_width(item.region) > flow.width(lane) =>
                    {
                        flow.next_page(
                            item.region,
                            &mut rows,
                            LayoutContinuation::measurement_refusal(),
                        );
                        advanced = true;
                        continue;
                    }
                    result => result?,
                };
                let height = spacing.box_height(&item, height, scale)?;
                if height > available_height {
                    return Err(FaceSceneError::InvalidExtent);
                }
                let cost =
                    row_commands(&item, region_start) + 2 * usize::from(flow.new_panel(lane));
                if flow.fits(lane, height, cost) {
                    break (lane, width, end, height, cost);
                }
                if advanced {
                    return Err(FaceSceneError::InvalidExtent);
                }
                let cause = flow
                    .pressure(lane, height, cost)
                    .ok_or(FaceSceneError::InvalidExtent)?;
                flow.next_page(item.region, &mut rows, cause);
                advanced = true;
            };
            if rows.len() == MAX_ROWS {
                return Err(FaceSceneError::DocumentBound);
            }
            let bounds = flow.bounds(lane, inset, width, height);
            let index = rows.len();
            reading_order.push((original, chunk, index));
            rows.push(Row {
                continuation: flow.continuation.take(),
                text: remaining[..end].into(),
                role: item.role,
                paint: item.paint,
                control: item.control,
                subject_index: item.subject_index,
                region: item.region,
                region_inset: if flow.span.is_some() { flow.edge } else { 0 },
                page: flow.page,
                region_start,
                insets: PackedRowInsets::new(insets)?,
                checkbox: item.checkbox,
                icon: item.icon,
                icon_size: super::super::semantic_icons::size(scale),
                panel_bounds: None,
                bounds,
            });
            chunk += 1;
            flow.commit(lane, index, height, cost);
            remaining = &remaining[end..];
            region_start = false;
        }
    }
    flow.finish_band(&mut rows);
    flow.finish_span();
    ordering::restore(&mut rows, &mut reading_order);
    Ok(rows)
}

#[cfg(test)]
#[path = "panels/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "panels/continuation_tests.rs"]
mod continuation_tests;
