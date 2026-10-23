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
    padding: u16,
) -> Result<u16, FaceSceneError> {
    width
        .checked_sub(padding * 2)
        .and_then(|width| width.checked_sub(super::super::semantic_icons::inset(item.icon)))
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
    pub padding: u16,
}

pub(super) struct Demand {
    height: u32,
    rows: usize,
    commands: usize,
}

impl Demand {
    pub fn fits(&self, y: u16, bottom: u16, rows: usize, commands: usize) -> bool {
        u32::from(y) + self.height <= u32::from(bottom)
            && rows + self.rows <= MAX_PAGE_ROWS
            && commands + self.commands <= conduit_face::MAX_GRAPHICS_COMMANDS
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
        let width = text_width(item, width, metrics.padding)?;
        let mut remaining = item.text.as_str();
        while !remaining.is_empty() {
            let (end, height) = measure_prefix(
                remaining,
                item.role,
                width,
                metrics.maximum_height,
                metrics.scale,
                metrics.typography,
            )?;
            demand.height += u32::from(height + metrics.padding * 2 + SPACE_SM);
            demand.rows += 1;
            demand.commands += row_commands(item, marker);
            marker = false;
            // This cluster cannot be kept on an empty page. Leave it to the
            // existing explicit page marker/Details continuation, without an
            // empty-page advance or repeated suffix measurement.
            if demand.rows > MAX_PAGE_ROWS
                || demand.commands + chrome::COMMANDS + 1 > conduit_face::MAX_GRAPHICS_COMMANDS
                || demand.height - u32::from(SPACE_SM)
                    > u32::from(metrics.maximum_height + metrics.padding * 2)
            {
                return Ok(None);
            }
            remaining = &remaining[end..];
        }
    }
    if item_count < 2 || demand.rows == 0 {
        return Ok(None);
    }
    demand.height -= u32::from(SPACE_SM);
    Ok(Some(demand))
}
