//! Safe accessible cuts carry exact row cost and inherited group insets.
//! Each fragment has at most 20 rows, so splitting/rebasing never scans or moves
//! an entire document suffix. Complete keep groups remain indivisible.
use super::*;
pub(super) struct Entry {
    pub original: usize,
    pub chunk: usize,
    pub row: Row,
    pub cross_extent: u16,
    pub cost: usize,
    pub break_before: bool,
    pub outer_padding: u16,
    pub extent: Option<BlockExtent>,
}
/// A grid band can include allocated track space beyond its last painted row.
/// Insets added outside the band repeat on continuation without losing that space.
pub(super) struct BlockExtent {
    pub origin: u16,
    pub end: u16,
    pub inset: u16,
}
impl Entry {
    pub fn translate(&mut self, x: u16, y: u16) {
        self.row.bounds.x += x as i16;
        self.row.bounds.y += y as i16;
        if let Some(extent) = &mut self.extent {
            extent.origin += y;
            extent.end += y;
        }
    }
    fn end(&self) -> u32 {
        let row = u32::from(self.row.bounds.y as u16)
            + u32::from(self.row.bounds.height)
            + u32::from(self.outer_padding);
        self.extent
            .as_ref()
            .map_or(row, |extent| row.max(u32::from(extent.end)))
    }
    fn origin(&self) -> u16 {
        let row = (self.row.bounds.y as u16).saturating_sub(self.outer_padding);
        self.extent.as_ref().map_or(row, |extent| {
            row.min(extent.origin.saturating_sub(extent.inset))
        })
    }
}
pub(super) struct Fragment {
    pub continuation: Option<LayoutContinuation>,
    pub entries: Vec<Entry>,
    pub height: u16,
    pub cost: usize,
}
impl Fragment {
    pub fn empty() -> Self {
        Self {
            continuation: None,
            entries: Vec::new(),
            height: 0,
            cost: 0,
        }
    }
    pub fn fits(&self, height: u16, rows: usize, cost: usize, limit: u16) -> bool {
        height <= limit
            && self.entries.len() + rows <= MAX_PAGE_ROWS
            && self.cost + cost + chrome::COMMANDS < conduit_face::MAX_GRAPHICS_COMMANDS
    }
    pub fn translate(&mut self, x: u16, y: u16) {
        for entry in &mut self.entries {
            entry.translate(x, y);
        }
    }
}

impl Fragment {
    /// Consume the largest safe prefix within the supplied residual budget.
    /// Each entry is inspected once until pressure. Keep groups and associated
    /// fresh-page-fit field clusters suppress their internal cut positions.
    pub fn take_prefix(&mut self, height: u16, rows: usize, cost: usize) -> Option<Self> {
        let mut used_cost = 0;
        let mut used_height = 0u32;
        let mut end = 0;
        let incoming = self.continuation;
        let mut pressure = None;
        for (index, entry) in self.entries.iter().enumerate() {
            used_cost += entry.cost;
            used_height = used_height.max(entry.end());
            if index >= rows || used_cost > cost || used_height > u32::from(height) {
                pressure = LayoutContinuation::pressure(
                    used_height > u32::from(height),
                    index >= rows,
                    used_cost > cost,
                )
                .map(|cause| {
                    if end < index
                        || !entry.break_before
                        || self
                            .entries
                            .get(index + 1)
                            .is_some_and(|next| !next.break_before)
                    {
                        cause.keeping_group()
                    } else {
                        cause
                    }
                });
                break;
            }
            if self
                .entries
                .get(index + 1)
                .is_none_or(|next| next.break_before)
            {
                end = index + 1;
            }
        }
        if end == 0 {
            self.continuation = LayoutContinuation::merge(incoming, pressure);
            return None;
        }
        self.continuation = pressure;
        let tail = self.entries.split_off(end);
        let entries = core::mem::replace(&mut self.entries, tail);
        let mut prefix = Self {
            continuation: incoming,
            entries,
            height: 0,
            cost: 0,
        };
        prefix.refresh();
        // Repeated group padding belongs on continuation pages too. Remove
        // prior flow offset, never the inherited structural inset.
        if let Some(shift) = self.entries.iter().map(Entry::origin).min() {
            for entry in &mut self.entries {
                entry.row.bounds.y -= shift as i16;
                if let Some(extent) = &mut entry.extent {
                    extent.origin -= shift;
                    extent.end -= shift;
                }
            }
        }
        self.refresh();
        Some(prefix)
    }
    fn refresh(&mut self) {
        self.cost = self.entries.iter().map(|entry| entry.cost).sum();
        self.height = self
            .entries
            .iter()
            .map(|entry| entry.end() as u16)
            .max()
            .unwrap_or(0);
    }
}
