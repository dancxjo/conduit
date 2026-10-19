//! Bounded visible-surface ordering and exact horizontal coverage intervals.
use super::{CompositorSurface, MAX_COMPOSITOR_SURFACES, NativeCompositorError};

pub(super) struct PreparedSurfaces<'a> {
    surfaces: &'a [CompositorSurface],
    order: [usize; MAX_COMPOSITOR_SURFACES],
    count: usize,
}
impl<'a> PreparedSurfaces<'a> {
    pub(super) fn new(surfaces: &'a [CompositorSurface]) -> Result<Self, NativeCompositorError> {
        if surfaces.len() > MAX_COMPOSITOR_SURFACES {
            return Err(NativeCompositorError::TooManySurfaces);
        }
        let mut prepared = Self {
            surfaces,
            order: [0; MAX_COMPOSITOR_SURFACES],
            count: 0,
        };
        for (index, surface) in surfaces.iter().enumerate() {
            if surface.visible && surface.is_raster_ready() {
                prepared.order[prepared.count] = index;
                prepared.count += 1;
            }
        }
        prepared.order[..prepared.count].sort_unstable_by_key(|&index| (surfaces[index].z, index));
        Ok(prepared)
    }
    pub(super) fn row(&self, left: u32, right: u32, y: u32) -> RowSpans<'_, 'a> {
        let mut edges = [0; 2 * MAX_COMPOSITOR_SURFACES + 2];
        edges[0] = left;
        edges[1] = right;
        let mut count = 2;
        for &index in &self.order[..self.count] {
            let bounds = self.surfaces[index].bounds;
            let top = i64::from(bounds.y);
            if i64::from(y) < top || i64::from(y) >= top + i64::from(bounds.height) {
                continue;
            }
            let start = i64::from(bounds.x).max(i64::from(left));
            let end = (i64::from(bounds.x) + i64::from(bounds.width)).min(i64::from(right));
            if start < end {
                edges[count] = start as u32;
                edges[count + 1] = end as u32;
                count += 2;
            }
        }
        edges[..count].sort_unstable();
        RowSpans {
            prepared: self,
            edges,
            count,
            next: 0,
            y,
        }
    }
}
pub(super) struct RowSpans<'p, 'a> {
    prepared: &'p PreparedSurfaces<'a>,
    edges: [u32; 2 * MAX_COMPOSITOR_SURFACES + 2],
    count: usize,
    next: usize,
    y: u32,
}
impl<'a> Iterator for RowSpans<'_, 'a> {
    type Item = (u32, u32, Option<(usize, &'a CompositorSurface)>);
    fn next(&mut self) -> Option<Self::Item> {
        while self.next + 1 < self.count {
            let left = self.edges[self.next];
            let right = self.edges[self.next + 1];
            self.next += 1;
            if left == right {
                continue;
            }
            let selected = self.prepared.order[..self.prepared.count]
                .iter()
                .rev()
                .find_map(|&index| {
                    let surface = &self.prepared.surfaces[index];
                    let bounds = surface.bounds;
                    let x = i64::from(left);
                    let y = i64::from(self.y);
                    (x >= i64::from(bounds.x)
                        && x < i64::from(bounds.x) + i64::from(bounds.width)
                        && y >= i64::from(bounds.y)
                        && y < i64::from(bounds.y) + i64::from(bounds.height))
                    .then_some((index, surface))
                });
            return Some((left, right, selected));
        }
        None
    }
}

/// One-pixel damage needs no sorted coverage preparation.
pub(super) fn select_at(
    surfaces: &[CompositorSurface],
    x: u32,
    y: u32,
) -> Option<(usize, &CompositorSurface)> {
    surfaces
        .iter()
        .enumerate()
        .filter(|(_, surface)| {
            let bounds = surface.bounds;
            let x = i64::from(x);
            let y = i64::from(y);
            surface.visible
                && surface.is_raster_ready()
                && x >= i64::from(bounds.x)
                && x < i64::from(bounds.x) + i64::from(bounds.width)
                && y >= i64::from(bounds.y)
                && y < i64::from(bounds.y) + i64::from(bounds.height)
        })
        .max_by_key(|(index, surface)| (surface.z, *index))
}
