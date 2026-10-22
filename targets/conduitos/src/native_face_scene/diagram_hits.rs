//! Exact reading targets from the same bounded traversal that paints the graph.
use super::*;

pub(in super::super) struct DiagramHits<'a> {
    pub gears: [Option<(LayoutRect, &'a str)>; 6],
    pub ports: [Option<(LayoutRect, &'a str)>; 24],
    pub cords: [Option<(GraphicsPath, &'a str)>; 12],
}

impl<'a> DiagramHits<'a> {
    pub fn new(gears: [Option<(LayoutRect, &'a str)>; 6]) -> Self {
        Self {
            gears,
            ports: [None; 24],
            cords: [None; 12],
        }
    }

    pub fn hit_test(&self, x: i16, y: i16) -> Option<&'a str> {
        // Ports occupy card rows. Cards cover paths painted underneath them.
        // Only uncovered, actually emitted Cord segments can be selected.
        for (bounds, identity) in self.ports.iter().chain(self.gears.iter()).flatten() {
            if i32::from(x) >= i32::from(bounds.x)
                && i32::from(y) >= i32::from(bounds.y)
                && i32::from(x) < i32::from(bounds.x) + i32::from(bounds.width)
                && i32::from(y) < i32::from(bounds.y) + i32::from(bounds.height)
            {
                return Some(identity);
            }
        }
        self.cords.iter().flatten().find_map(|(path, identity)| {
            path.points()
                .windows(2)
                .any(|pair| {
                    let (ax, ay) = (i32::from(*pair[0].x()), i32::from(*pair[0].y()));
                    let (bx, by) = (i32::from(*pair[1].x()), i32::from(*pair[1].y()));
                    let (x, y) = (i32::from(x), i32::from(y));
                    if ax == bx {
                        (x - ax).abs() <= 4 && y >= ay.min(by) && y <= ay.max(by)
                    } else {
                        (y - ay).abs() <= 4 && x >= ax.min(bx) && x <= ax.max(bx)
                    }
                })
                .then_some(*identity)
        })
    }
}
