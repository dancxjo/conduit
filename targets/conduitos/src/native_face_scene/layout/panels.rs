//! Finite adjacent lanes for the first authored pair of checked groups.
//! Original rows/actions never move in the document vector. Later groups and
//! unmatched reading span the available width; only geometry is assigned here.
use super::*;

#[path = "panels/ordering.rs"]
mod ordering;
const EDGE: u16 = SPACE_SM;

struct Band {
    top: u16,
    next: [u16; 2],
    first: [Option<usize>; 2],
}

struct Flow {
    groups: [u8; 2],
    gutter: u16,
    width: u16,
    column: u16,
    single_lane: bool,
    paired_regions: bool,
    top: u16,
    bottom: u16,
    y: u16,
    band: Option<Band>,
    page: usize,
    count: usize,
    commands: usize,
}

impl Flow {
    fn new(
        screen: LayoutRect,
        gutter: u16,
        scale: u16,
        groups: [u8; 2],
        typography: Typography,
        paired_regions: bool,
    ) -> Result<Self, FaceSceneError> {
        let chrome = chrome::Chrome::prepare_selected(screen, scale, typography)?;
        let width = screen
            .width
            .checked_sub(gutter.checked_mul(2).ok_or(FaceSceneError::InvalidExtent)?)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let column = width
            .checked_sub(gutter)
            .map(|value| value / 2)
            .filter(|value| *value > EDGE * 2)
            .ok_or(FaceSceneError::InvalidExtent)?;
        Ok(Self {
            groups,
            gutter,
            width,
            column,
            single_lane: !paired_regions,
            paired_regions,
            top: chrome.top,
            bottom: chrome.bottom,
            y: chrome.top,
            band: None,
            page: 0,
            count: 0,
            commands: chrome::COMMANDS + 1,
        })
    }

    fn lane(&self, region: Option<u8>) -> Option<usize> {
        region.and_then(|region| {
            self.groups
                .iter()
                .position(|group| *group == region)
                .map(|lane| if self.single_lane { 0 } else { lane })
        })
    }

    fn panel_width(&self) -> u16 {
        if self.single_lane {
            self.width
        } else {
            self.column
        }
    }

    fn continuation_width(&self, region: Option<u8>) -> u16 {
        if region == Some(self.groups[1]) {
            self.width - EDGE * 2
        } else {
            self.width(self.lane(region))
        }
    }

    fn finish_band(&mut self, rows: &mut [Row]) {
        let Some(band) = self.band.take() else {
            return;
        };
        let column = self.panel_width();
        for lane in 0..2 {
            if let Some(first) = band.first[lane] {
                rows[first].panel_bounds = Some(LayoutRect {
                    x: (self.gutter + lane as u16 * (column + self.gutter)) as i16,
                    y: band.top as i16,
                    width: column,
                    height: band.next[lane] - SPACE_SM + EDGE - band.top,
                });
            }
        }
        self.y = if band.first.iter().any(Option::is_some) {
            band.next.into_iter().max().unwrap_or(band.top) + EDGE
        } else {
            band.top
        };
    }

    fn select(&mut self, lane: Option<usize>, rows: &mut [Row]) {
        if lane.is_none() {
            self.finish_band(rows);
        } else if self.band.is_none() {
            self.band = Some(Band {
                top: self.y,
                next: [self.y + EDGE; 2],
                first: [None; 2],
            });
        }
    }

    fn y(&self, lane: Option<usize>) -> u16 {
        lane.map_or(self.y, |lane| self.band.as_ref().unwrap().next[lane])
    }

    fn width(&self, lane: Option<usize>) -> u16 {
        if lane.is_some() {
            self.panel_width() - EDGE * 2
        } else {
            self.width
        }
    }

    fn new_panel(&self, lane: Option<usize>) -> bool {
        lane.is_some_and(|lane| self.band.as_ref().unwrap().first[lane].is_none())
    }

    fn fits(&self, lane: Option<usize>, height: u16, cost: usize) -> bool {
        u32::from(self.y(lane))
            + u32::from(height)
            + if lane.is_some() { u32::from(EDGE) } else { 0 }
            <= u32::from(self.bottom)
            && self.count < MAX_PAGE_ROWS
            && self.commands + cost <= conduit_face::MAX_GRAPHICS_COMMANDS
    }

    fn next_page(&mut self, region: Option<u8>, rows: &mut [Row]) {
        self.finish_band(rows);
        // Geometry is fixed before committing the first row of a page. Ordering
        // exhausts the first region before the second: only the latter remains
        // when its continuation starts, so it no longer reserves an empty lane.
        self.page += usize::from(self.count != 0);
        self.single_lane = !self.paired_regions || region == Some(self.groups[1]);
        self.count = 0;
        self.commands = chrome::COMMANDS + 1;
        self.y = self.top;
        self.select(self.lane(region), rows);
    }

    fn bounds(&self, lane: Option<usize>, inset: u16, width: u16, height: u16) -> LayoutRect {
        let x = self.gutter
            + lane.map_or(0, |lane| {
                lane as u16 * (self.panel_width() + self.gutter) + EDGE
            })
            + inset;
        LayoutRect {
            x: x as i16,
            y: self.y(lane) as i16,
            width,
            height,
        }
    }

    fn commit(&mut self, lane: Option<usize>, row: usize, height: u16, cost: usize) {
        if let Some(lane) = lane {
            let band = self.band.as_mut().unwrap();
            band.first[lane].get_or_insert(row);
            band.next[lane] += height + SPACE_SM;
        } else {
            self.y += height + SPACE_SM;
        }
        self.count += 1;
        self.commands += cost;
    }
}

pub(super) fn admit(
    items: Vec<document::Item>,
    screen: LayoutRect,
    gutter: u16,
    scale: u16,
    padding: u16,
    groups: [u8; 2],
    typography: Typography,
) -> Result<Vec<Row>, FaceSceneError> {
    let items = ordering::prepare(items, groups)?;
    // Empty authored regions do not reserve space. This decision is derived
    // from admitted rows, without changing source subjects or reading order.
    let paired_regions = groups.iter().all(|group| {
        items
            .iter()
            .any(|(_, item)| item.region == Some(*group) && !item.text.is_empty())
    });
    let mut flow = Flow::new(screen, gutter, scale, groups, typography, paired_regions)?;
    let padding = fit_padding(
        items.iter().map(|(_, item)| item),
        (flow.bottom - flow.top).saturating_sub(EDGE * 2),
        scale,
        padding,
        typography,
    );
    let mut rows = Vec::new();
    let mut reading_order = Vec::new();
    let mut bytes = 0usize;
    let mut previous_region = None;
    let mut items = items.into_iter();
    let mut previous_item = None;
    while let Some((original, item)) = items.next() {
        let cluster_start = field_clusters::starts(previous_item, &item);
        previous_item = field_clusters::key(&item);
        bytes = bytes
            .checked_add(item.text.len())
            .filter(|value| *value <= MAX_DOCUMENT_BYTES)
            .ok_or(FaceSceneError::DocumentBound)?;
        let lane = flow.lane(item.region);
        flow.select(lane, &mut rows);
        let inset = u16::from(item.indent) * INDENT_STEP;
        let maximum_height = flow
            .bottom
            .checked_sub(flow.top)
            .and_then(|value| {
                value.checked_sub(padding * 2 + if lane.is_some() { EDGE * 2 } else { 0 })
            })
            .filter(|value| *value > 0)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let mut region_start = item.region.is_some() && item.region != previous_region;
        previous_region = item.region;
        if cluster_start {
            let measure = |width| {
                field_clusters::measure(
                    core::iter::once(&item).chain(items.as_slice().iter().map(|(_, item)| item)),
                    field_clusters::Metrics {
                        width,
                        maximum_height,
                        scale,
                        typography,
                        padding,
                    },
                    region_start,
                )
            };
            let edge = if lane.is_some() { EDGE } else { 0 };
            let bottom = flow.bottom - edge;
            let current_fits = measure(flow.width(lane))?.is_some_and(|demand| {
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
                flow.next_page(item.region, &mut rows);
            }
        }

        let mut remaining = item.text.as_str();
        let mut chunk = 0usize;
        while !remaining.is_empty() {
            let mut advanced = false;
            let (lane, width, end, height, cost) = loop {
                let lane = flow.lane(item.region);
                let width = flow
                    .width(lane)
                    .checked_sub(inset)
                    .ok_or(FaceSceneError::InvalidExtent)?;
                let text_width = field_clusters::text_width(&item, width, padding)?;
                let (end, height) = measure_prefix(
                    remaining,
                    item.role,
                    text_width,
                    maximum_height,
                    scale,
                    typography,
                )?;
                let height = height + padding * 2;
                let cost =
                    row_commands(&item, region_start) + 2 * usize::from(flow.new_panel(lane));
                if flow.fits(lane, height, cost) {
                    break (lane, width, end, height, cost);
                }
                if advanced {
                    return Err(FaceSceneError::InvalidExtent);
                }
                flow.next_page(item.region, &mut rows);
                advanced = true;
            };
            if rows.len() == MAX_ROWS {
                return Err(FaceSceneError::DocumentBound);
            }
            let bounds = flow.bounds(lane, inset, width, height);
            let index = rows.len();
            reading_order.push((original, chunk, index));
            rows.push(Row {
                text: remaining[..end].into(),
                role: item.role,
                paint: item.paint,
                control: item.control,
                subject_index: item.subject_index,
                page: flow.page,
                region_start,
                padding,
                checkbox: item.checkbox,
                icon: item.icon,
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
    ordering::restore(&mut rows, &mut reading_order);
    Ok(rows)
}

#[cfg(test)]
#[path = "panels/tests.rs"]
mod tests;
