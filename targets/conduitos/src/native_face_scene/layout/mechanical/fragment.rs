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
}
pub(super) struct Fragment {
    pub entries: Vec<Entry>,
    pub height: u16,
    pub cost: usize,
}
impl Fragment {
    pub fn empty() -> Self {
        Self {
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
            entry.row.bounds.x += x as i16;
            entry.row.bounds.y += y as i16;
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
        for (index, entry) in self.entries.iter().enumerate() {
            used_cost += entry.cost;
            used_height = used_height.max(
                u32::from(entry.row.bounds.y as u16)
                    + u32::from(entry.row.bounds.height)
                    + u32::from(entry.outer_padding),
            );
            if index >= rows || used_cost > cost || used_height > u32::from(height) {
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
            return None;
        }
        let tail = self.entries.split_off(end);
        let entries = core::mem::replace(&mut self.entries, tail);
        let mut prefix = Self {
            entries,
            height: 0,
            cost: 0,
        };
        prefix.refresh();
        // Repeated group padding belongs on continuation pages too. Remove
        // prior flow offset, never the inherited structural inset.
        if let Some(shift) = self
            .entries
            .iter()
            .map(|entry| (entry.row.bounds.y as u16).saturating_sub(entry.outer_padding))
            .min()
        {
            for entry in &mut self.entries {
                entry.row.bounds.y -= shift as i16;
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
            .map(|entry| entry.row.bounds.y as u16 + entry.row.bounds.height + entry.outer_padding)
            .max()
            .unwrap_or(0);
    }
}
