//! Native bounded coverage intervals: fixed edges, ordered top-surface selection.
use crate::*;
pub(crate) struct Intervals {
    edges: [u32; 24],
    count: usize,
    next: usize,
    order: [usize; MAX_SURFACES],
    surfaces: [Surface; MAX_SURFACES],
    active: usize,
    y: u32,
}
impl Compositor<'_> {
    pub(crate) fn intervals(
        &self,
        left: u32,
        right: u32,
        y: u32,
        focus: Option<Rect>,
    ) -> Intervals {
        let mut row = Intervals {
            edges: [0; 24],
            count: 2,
            next: 0,
            order: [0; MAX_SURFACES],
            surfaces: self.surfaces,
            active: 0,
            y,
        };
        row.edges[0] = left;
        row.edges[1] = right;
        for (i, s) in self.surfaces.iter().enumerate() {
            if !s.live || !s.visible || !s.ready {
                continue;
            }
            row.order[row.active] = i;
            row.active += 1;
            if i64::from(y) < i64::from(s.rect.y)
                || i64::from(y) >= i64::from(s.rect.y) + i64::from(s.rect.height)
            {
                continue;
            }
            row.edge(i64::from(s.rect.x), left, right);
            row.edge(i64::from(s.rect.x) + i64::from(s.rect.width), left, right);
        }
        if let Some(r) = focus {
            for edge in [
                i64::from(r.x),
                i64::from(r.x) + 1,
                i64::from(r.x) + i64::from(r.width) - 1,
                i64::from(r.x) + i64::from(r.width),
            ] {
                row.edge(edge, left, right);
            }
        }
        if let Some((x, _)) = self.cursor {
            row.edge(i64::from(x), left, right);
            row.edge(i64::from(x) + 10, left, right);
        }
        row.order[..row.active]
            .sort_unstable_by_key(|&i| (self.surfaces[i].z, self.surfaces[i].order));
        row.edges[..row.count].sort_unstable();
        row
    }
}
impl Intervals {
    fn edge(&mut self, x: i64, left: u32, right: u32) {
        if x > i64::from(left) && x < i64::from(right) {
            self.edges[self.count] = x as u32;
            self.count += 1;
        }
    }
}
impl Iterator for Intervals {
    type Item = (u32, u32, Option<usize>);
    fn next(&mut self) -> Option<Self::Item> {
        while self.next + 1 < self.count {
            let left = self.edges[self.next];
            let right = self.edges[self.next + 1];
            self.next += 1;
            if left == right {
                continue;
            }
            let selected = self.order[..self.active]
                .iter()
                .rev()
                .copied()
                .find(|&i| self.surfaces[i].rect.contains(left, self.y));
            return Some((left, right, selected));
        }
        None
    }
}
