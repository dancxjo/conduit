//! Finite card geometry shared by drawing, endpoint visibility and inspection.
use super::super::{LayoutRect, SPACE_XL, layout};

pub(super) const GAP: u16 = 16;
pub(super) const PORT_ROW: u16 = 27;
pub(super) type GearHits<'a> = [Option<(LayoutRect, &'a str)>; 6];

pub(super) struct DiagramGeometry {
    pub columns: usize,
    pub rows: usize,
    pub width: u16,
    pub height: u16,
}

impl DiagramGeometry {
    pub fn new(screen: LayoutRect, gears: usize) -> Self {
        let columns = if screen.width >= 520 { 2 } else { 1 };
        let available = if screen.height >= 680 {
            3
        } else if screen.height >= 440 {
            2
        } else {
            1
        };
        let rows = available.min(gears.div_ceil(columns).max(1));
        let width =
            ((screen.width - SPACE_XL * 2 - GAP * (columns as u16 - 1)) / columns as u16).min(320);
        let height =
            ((screen.height - layout::HEADER - layout::FOOTER - 16 - GAP * (rows as u16 - 1))
                / rows as u16)
                .min(192);
        Self {
            columns,
            rows,
            width,
            height,
        }
    }

    pub fn capacity(&self) -> usize {
        self.columns * self.rows
    }

    pub fn card(&self, index: usize) -> LayoutRect {
        LayoutRect {
            x: (SPACE_XL + index.rem_euclid(self.columns) as u16 * (self.width + GAP)) as i16,
            y: (layout::HEADER + 8 + index.div_euclid(self.columns) as u16 * (self.height + GAP))
                as i16,
            width: self.width,
            height: self.height,
        }
    }
}

pub(super) fn visible_ports(height: u16, count: usize) -> usize {
    let maximum = usize::from(height.saturating_sub(78) / PORT_ROW).min(4);
    let reserve = if count > maximum { PORT_ROW } else { 0 };
    usize::from(height.saturating_sub(78 + reserve) / PORT_ROW).min(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overflow_notice_never_overlaps_visible_port_rows() {
        for height in 112..=192 {
            for count in 0..=128 {
                let shown = visible_ports(height, count).min(count);
                if count > shown && shown > 0 {
                    let last_bottom = 70 + (shown as u16 - 1) * PORT_ROW + 23;
                    assert!(last_bottom <= height - 26);
                }
            }
        }
        assert_eq!(visible_ports(112, 1), 1);
        assert_eq!(visible_ports(112, 2), 0);
        assert_eq!(visible_ports(192, 5), 3);
    }
}
