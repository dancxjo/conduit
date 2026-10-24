//! Prepared Lucide coverage on an eighth-pixel lattice. No SVG parser,
//! allocation, floating point, or geometry-dependent work occurs during Play.
use super::Icon;

#[path = "lucide_index.rs"]
mod generated;

const EDGE: usize = 192;
const BYTES_PER_ICON: usize = EDGE * EDGE / 8;
const MASKS: &[u8; Icon::ALL.len() * BYTES_PER_ICON] =
    include_bytes!("../../../assets/icons/lucide-192.bin");

pub(super) const fn ink(icon: Icon, x: i64, y: i64) -> bool {
    if x < 0 || y < 0 || x >= EDGE as i64 || y >= EDGE as i64 {
        return false;
    }
    let index = generated::index(icon);
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
