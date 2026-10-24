//! Prepared Lucide coverage on an eighth-pixel lattice. No SVG parser,
//! allocation, floating point, or geometry-dependent work occurs during Play.
use super::Icon;

const EDGE: usize = 192;
const BYTES_PER_ICON: usize = EDGE * EDGE / 8;
const MASKS: &[u8; 23 * BYTES_PER_ICON] = include_bytes!("../../../assets/icons/lucide-192.bin");

pub(super) const fn ink(icon: Icon, x: i64, y: i64) -> bool {
    if x < 0 || y < 0 || x >= EDGE as i64 || y >= EDGE as i64 {
        return false;
    }
    let index = match icon {
        Icon::Clock => 0,
        Icon::Repeat2 => 1,
        Icon::Face => 2,
        Icon::Type => 3,
        Icon::CaseUpper => 4,
        Icon::Combine => 5,
        Icon::Tally5 => 6,
        Icon::ChartColumnsIncreasing => 7,
        Icon::FileOutput => 8,
        Icon::Keyboard => 9,
        Icon::GenericGear => 10,
        Icon::Body => 11,
        Icon::Wake => 12,
        Icon::Plan => 13,
        Icon::Play => 14,
        Icon::Host => 15,
        Icon::Port => 16,
        Icon::Line => 17,
        Icon::Status => 18,
        Icon::Warning => 19,
        Icon::Close => 20,
        Icon::Back => 21,
        Icon::Confirm => 22,
    };
    let bit = y as usize * EDGE + x as usize;
    MASKS[index * BYTES_PER_ICON + bit / 8] & (1 << (bit % 8)) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_icon_has_finite_nonempty_coverage_and_empty_outside_bounds() {
        for icon in Icon::ALL {
            let pixels = (0..192)
                .flat_map(|y| (0..192).map(move |x| (x, y)))
                .filter(|&(x, y)| ink(icon, x, y))
                .count();
            assert!((64..192 * 192 / 2).contains(&pixels), "{icon:?}: {pixels}");
            for (x, y) in [(-1, 0), (0, -1), (192, 0), (0, 192), (i64::MAX, i64::MIN)] {
                assert!(!ink(icon, x, y));
            }
        }
    }
}
