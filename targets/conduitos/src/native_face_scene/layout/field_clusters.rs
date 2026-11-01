//! Bounded keep decisions for contiguous rows associated with one Face target.
//! Oversized targets keep the ordinary finite row/page continuation path.
use super::*;

pub(super) fn validate(items: &[document::Item]) -> Result<(), FaceSceneError> {
    if items.len() > MAX_ROWS
        || items
            .iter()
            .try_fold(0usize, |sum, item| {
                sum.checked_add(item.text.len())
                    .filter(|sum| *sum <= MAX_DOCUMENT_BYTES)
            })
            .is_none()
    {
        return Err(FaceSceneError::DocumentBound);
    }
    Ok(())
}

fn same(a: &document::Item, b: &document::Item) -> bool {
    a.subject_index.is_some()
        && a.subject_index == b.subject_index
        && a.region == b.region
        && a.region_order == b.region_order
}

pub(super) type Key = (usize, Option<u8>, Option<usize>);

pub(super) fn key(item: &document::Item) -> Option<Key> {
    item.subject_index
        .map(|subject| (subject, item.region, item.region_order))
}

pub(super) fn starts(previous: Option<Key>, current: &document::Item) -> bool {
    key(current).is_some_and(|key| Some(key) != previous)
}

pub(super) fn text_width(
    item: &document::Item,
    width: u16,
    insets: RowInsets,
    scale: u16,
) -> Result<u16, FaceSceneError> {
    width
        .checked_sub(insets.inline * 2 + insets.leading)
        .and_then(|width| {
            width.checked_sub(super::super::semantic_icons::inset(
                item.icon,
                super::super::semantic_icons::size(scale),
            ))
        })
        .and_then(|width| {
            width.checked_sub(if item.checkbox.is_some() {
                super::super::boolean_choice::INSET
            } else {
                0
            })
        })
        .filter(|width| *width > 0)
        .ok_or(FaceSceneError::InvalidExtent)
}

pub(super) struct Metrics {
    pub width: u16,
    pub maximum_height: u16,
    pub scale: u16,
    pub typography: Typography,
    pub spacing: RowSpacing,
}

pub(super) struct Demand {
    height: u32,
    rows: usize,
    commands: usize,
}

impl Demand {
    pub fn fits(&self, y: u16, bottom: u16, rows: usize, commands: usize) -> bool {
        self.pressure(y, bottom, rows, commands).is_none()
    }

    pub fn pressure(
        &self,
        y: u16,
        bottom: u16,
        rows: usize,
        commands: usize,
    ) -> Option<LayoutContinuation> {
        LayoutContinuation::pressure(
            u32::from(y) + self.height > u32::from(bottom),
            rows + self.rows > MAX_PAGE_ROWS,
            commands + self.commands > conduit_face::MAX_GRAPHICS_COMMANDS,
        )
    }
}

/// Called only at a run start. Scanning is linear in admitted items; measuring
/// stops after a fresh page's height/20 rows/48 commands. No scratch allocation.
pub(super) fn measure<'a>(
    items: impl Iterator<Item = &'a document::Item> + Clone,
    metrics: Metrics,
    region_start: bool,
) -> Result<Option<Demand>, FaceSceneError> {
    let Some(first) = items.clone().next() else {
        return Ok(None);
    };
    let cluster = items.take_while(|item| same(first, item));
    if !cluster.clone().any(|item| item.control.is_some()) {
        return Ok(None);
    }
    measure_cluster(cluster, metrics, region_start)
}

/// An oversized subject may continue its prose while retaining a fitting
/// contiguous action and its own arguments as one smaller interactive group.
pub(super) fn measure_controls<'a>(
    items: impl Iterator<Item = &'a document::Item> + Clone,
    metrics: Metrics,
    region_start: bool,
) -> Result<Option<Demand>, FaceSceneError> {
    let Some(first) = items.clone().next() else {
        return Ok(None);
    };
    let Some(control) = first.control.filter(|control| control.argument.is_none()) else {
        return Ok(None);
    };
    let cluster = items.take_while(|item| {
        same(first, item)
            && item
                .control
                .is_some_and(|next| next.action == control.action)
    });
    measure_cluster(cluster, metrics, region_start)
}

fn measure_cluster<'a>(
    cluster: impl Iterator<Item = &'a document::Item>,
    metrics: Metrics,
    region_start: bool,
) -> Result<Option<Demand>, FaceSceneError> {
    let mut demand = Demand {
        height: 0,
        rows: 0,
        commands: 0,
    };
    let mut item_count = 0;
    let mut marker = region_start;
    for item in cluster {
        item_count += 1;
        let width = metrics
            .width
            .checked_sub(u16::from(item.indent) * INDENT_STEP)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let insets = metrics.spacing.insets(item);
        let maximum_height = metrics
            .maximum_height
            .checked_sub(insets.block * 2)
            .filter(|height| *height > 0)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let width = text_width(item, width, insets, metrics.scale)?;
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let (end, height) = measure_prefix(
                remaining,
                item.role,
                width,
                maximum_height,
                metrics.scale,
                metrics.typography,
            )?;
            demand.height += u32::from(
                metrics.spacing.box_height(item, height, metrics.scale)? + metrics.spacing.gap,
            );
            demand.rows += 1;
            demand.commands += row_commands(item, marker);
            marker = false;
            // This cluster cannot be kept on an empty page. Leave it to the
            // existing explicit page marker/Details continuation, without an
            // empty-page advance or repeated suffix measurement.
            if demand.rows > MAX_PAGE_ROWS
                || demand.commands + chrome::COMMANDS + 1 > conduit_face::MAX_GRAPHICS_COMMANDS
                || demand.height - u32::from(metrics.spacing.gap)
                    > u32::from(metrics.maximum_height)
            {
                return Ok(None);
            }
            remaining = &remaining[end..];
        }
    }
    if item_count < 2 || demand.rows == 0 {
        return Ok(None);
    }
    demand.height -= u32::from(metrics.spacing.gap);
    Ok(Some(demand))
}
