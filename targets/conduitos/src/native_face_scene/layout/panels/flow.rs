//! Admitted panel viewport, continuation and finite per-page pressure.
use super::*;

struct Band {
    top: u16,
    next: [u16; 2],
    first: [Option<usize>; 2],
}

pub(super) struct Flow {
    groups: Option<[u8; 2]>,
    gutter: u16,
    gap: u16,
    pub(super) edge: u16,
    styled: bool,
    pub(super) span: Option<(u8, bool)>,
    width: u16,
    column: u16,
    single_lane: bool,
    paired_regions: bool,
    pub(super) top: u16,
    pub(super) bottom: u16,
    y: u16,
    band: Option<Band>,
    pub(super) page: usize,
    pub(super) count: usize,
    pub(super) commands: usize,
    pub(super) continuation: Option<LayoutContinuation>,
}

impl Flow {
    pub(super) fn new(
        screen: LayoutRect,
        gutter: u16,
        scale: u16,
        groups: Option<[u8; 2]>,
        typography: Typography,
        paired_regions: bool,
        spacing: RowSpacing,
    ) -> Result<Self, FaceSceneError> {
        let edge = spacing.panel.unwrap_or(LEGACY_EDGE);
        let chrome = chrome::Chrome::prepare_selected(screen, scale, typography)?;
        let width = screen
            .width
            .checked_sub(gutter.checked_mul(2).ok_or(FaceSceneError::InvalidExtent)?)
            .ok_or(FaceSceneError::InvalidExtent)?;
        let column = if groups.is_some() {
            width.checked_sub(gutter).map(|value| value / 2)
        } else {
            Some(width)
        }
        .filter(|value| *value > edge * 2)
        .ok_or(FaceSceneError::InvalidExtent)?;
        Ok(Self {
            groups,
            gutter,
            gap: spacing.gap,
            edge,
            styled: spacing.panel.is_some(),
            span: None,
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
            continuation: None,
        })
    }

    pub(super) fn lane(&self, region: Option<u8>) -> Option<usize> {
        region.and_then(|region| {
            self.groups
                .as_ref()?
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

    pub(super) fn continuation_width(&self, region: Option<u8>) -> u16 {
        if self.groups.is_some_and(|groups| region == Some(groups[1])) {
            self.width - self.edge * 2
        } else {
            self.width(self.lane(region))
        }
    }

    pub(super) fn finish_band(&mut self, rows: &mut [Row]) {
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
                    height: band.next[lane] - self.gap + self.edge - band.top,
                });
            }
        }
        self.y = if band.first.iter().any(Option::is_some) {
            band.next.into_iter().max().unwrap_or(band.top) + self.edge
        } else {
            band.top
        };
    }

    pub(super) fn finish_span(&mut self) {
        if let Some((_, content)) = self.span.take() {
            if content {
                self.y += self.edge;
            } else {
                self.y -= self.edge;
            }
        }
    }

    pub(super) fn select(&mut self, lane: Option<usize>, region: Option<u8>, rows: &mut [Row]) {
        if lane.is_none() {
            self.finish_band(rows);
            let span = region.filter(|_| self.styled);
            if self.span.map(|(region, _)| region) != span {
                self.finish_span();
                if let Some(region) = span {
                    self.y += self.edge;
                    self.span = Some((region, false));
                }
            }
        } else {
            self.finish_span();
            if self.band.is_none() {
                self.band = Some(Band {
                    top: self.y,
                    next: [self.y + self.edge; 2],
                    first: [None; 2],
                });
            }
        }
    }

    pub(super) fn y(&self, lane: Option<usize>) -> u16 {
        lane.map_or(self.y, |lane| self.band.as_ref().unwrap().next[lane])
    }

    pub(super) fn width(&self, lane: Option<usize>) -> u16 {
        if lane.is_some() {
            self.panel_width() - self.edge * 2
        } else if self.span.is_some() {
            self.width - self.edge * 2
        } else {
            self.width
        }
    }

    pub(super) fn new_panel(&self, lane: Option<usize>) -> bool {
        lane.is_some_and(|lane| self.band.as_ref().unwrap().first[lane].is_none())
    }

    pub(super) fn fits(&self, lane: Option<usize>, height: u16, cost: usize) -> bool {
        self.pressure(lane, height, cost).is_none()
    }

    pub(super) fn pressure(
        &self,
        lane: Option<usize>,
        height: u16,
        cost: usize,
    ) -> Option<LayoutContinuation> {
        LayoutContinuation::pressure(
            u32::from(self.y(lane))
                + u32::from(height)
                + if lane.is_some() || self.span.is_some() {
                    u32::from(self.edge)
                } else {
                    0
                }
                > u32::from(self.bottom),
            self.count >= MAX_PAGE_ROWS,
            self.commands + cost > conduit_face::MAX_GRAPHICS_COMMANDS,
        )
    }

    pub(super) fn next_page(
        &mut self,
        region: Option<u8>,
        rows: &mut [Row],
        cause: LayoutContinuation,
    ) {
        self.finish_band(rows);
        self.finish_span();
        // Geometry is fixed before committing the first row of a page. Ordering
        // exhausts the first region before the second: only the latter remains
        // when its continuation starts, so it no longer reserves an empty lane.
        if self.count != 0 {
            self.page += 1;
            self.continuation = Some(cause);
        }
        self.single_lane =
            !self.paired_regions || self.groups.is_some_and(|groups| region == Some(groups[1]));
        self.count = 0;
        self.commands = chrome::COMMANDS + 1;
        self.y = self.top;
        self.select(self.lane(region), region, rows);
    }

    pub(super) fn bounds(
        &self,
        lane: Option<usize>,
        inset: u16,
        width: u16,
        height: u16,
    ) -> LayoutRect {
        let x = self.gutter
            + lane.map_or(if self.span.is_some() { self.edge } else { 0 }, |lane| {
                lane as u16 * (self.panel_width() + self.gutter) + self.edge
            })
            + inset;
        LayoutRect {
            x: x as i16,
            y: self.y(lane) as i16,
            width,
            height,
        }
    }

    pub(super) fn commit(&mut self, lane: Option<usize>, row: usize, height: u16, cost: usize) {
        if let Some(lane) = lane {
            let band = self.band.as_mut().unwrap();
            band.first[lane].get_or_insert(row);
            band.next[lane] += height + self.gap;
        } else {
            if let Some((_, content)) = &mut self.span {
                *content = true;
            }
            self.y += height + self.gap;
        }
        self.count += 1;
        self.commands += cost;
    }
}
