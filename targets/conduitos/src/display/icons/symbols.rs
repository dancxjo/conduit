//! Compile-time raster resources for the shared native symbol vocabulary.
use super::{EDGE, frame, line, ring};
use conduit_presentation::GraphicsSymbol;

const fn ink(symbol: GraphicsSymbol, x: i32, y: i32) -> bool {
    use GraphicsSymbol::*;
    match symbol {
        Form => frame(x, y, 5, 2, 19, 22) || line(x, y, 8, 8, 16, 8) || line(x, y, 8, 12, 16, 12),
        Build => line(x, y, 4, 20, 18, 6) || frame(x, y, 12, 2, 21, 8),
        Body => ring(x, y, 12, 6, 3) || frame(x, y, 5, 13, 19, 21),
        Wake => {
            ring(x, y, 12, 12, 6)
                || line(x, y, 12, 1, 12, 4)
                || line(x, y, 12, 20, 12, 23)
                || line(x, y, 1, 12, 4, 12)
                || line(x, y, 20, 12, 23, 12)
        }
        Lull => {
            ring(x, y, 12, 12, 9) && x < 13 || line(x, y, 13, 3, 8, 12) || line(x, y, 8, 12, 13, 21)
        }
        Plan => {
            frame(x, y, 3, 3, 8, 8)
                || frame(x, y, 16, 16, 21, 21)
                || line(x, y, 8, 6, 18, 6)
                || line(x, y, 18, 6, 18, 16)
        }
        Play => x >= 6 && x <= 20 && y >= 3 && y <= 21 && (y - 12).abs() * 14 <= (20 - x) * 9,
        Stop => x >= 5 && x <= 19 && y >= 5 && y <= 19,
        Hold => (x >= 5 && x <= 9 || x >= 15 && x <= 19) && y >= 4 && y <= 20,
        Host => {
            frame(x, y, 3, 3, 21, 16) || line(x, y, 12, 16, 12, 21) || line(x, y, 7, 21, 17, 21)
        }
        Gear => {
            ring(x, y, 12, 12, 8)
                || ring(x, y, 12, 12, 3)
                || line(x, y, 12, 1, 12, 5)
                || line(x, y, 12, 19, 12, 23)
        }
        PortInput => {
            ring(x, y, 8, 12, 5)
                || line(x, y, 14, 12, 22, 12)
                || line(x, y, 14, 12, 18, 8)
                || line(x, y, 14, 12, 18, 16)
        }
        PortOutput => {
            ring(x, y, 8, 12, 5)
                || line(x, y, 14, 12, 22, 12)
                || line(x, y, 22, 12, 18, 8)
                || line(x, y, 22, 12, 18, 16)
        }
        Cord => ring(x, y, 4, 17, 2) || ring(x, y, 20, 7, 2) || line(x, y, 6, 17, 18, 7),
        Line => frame(x, y, 1, 13, 7, 19) || frame(x, y, 17, 5, 23, 11) || line(x, y, 7, 16, 17, 8),
        Sign => frame(x, y, 3, 3, 21, 16) || line(x, y, 7, 16, 7, 21) || line(x, y, 7, 21, 12, 16),
        Info => ring(x, y, 12, 12, 10) || line(x, y, 12, 10, 12, 18) || ring(x, y, 12, 6, 1),
        Face => {
            frame(x, y, 3, 3, 21, 21)
                || ring(x, y, 8, 9, 1)
                || ring(x, y, 16, 9, 1)
                || line(x, y, 8, 16, 16, 16)
        }
        Back => line(x, y, 3, 12, 21, 12) || line(x, y, 3, 12, 10, 5) || line(x, y, 3, 12, 10, 19),
        Warning => {
            line(x, y, 12, 2, 2, 21)
                || line(x, y, 2, 21, 22, 21)
                || line(x, y, 22, 21, 12, 2)
                || line(x, y, 12, 8, 12, 14)
                || ring(x, y, 12, 18, 1)
        }
        Failure => line(x, y, 5, 5, 19, 19) || line(x, y, 19, 5, 5, 19),
        Success => line(x, y, 3, 12, 9, 18) || line(x, y, 9, 18, 21, 5),
        Open => {
            frame(x, y, 2, 8, 22, 21)
                || line(x, y, 2, 8, 2, 4)
                || line(x, y, 2, 4, 10, 4)
                || line(x, y, 10, 4, 14, 8)
        }
        Save => frame(x, y, 3, 2, 21, 22) || frame(x, y, 7, 2, 17, 9) || frame(x, y, 7, 15, 17, 22),
        Inspect => ring(x, y, 10, 10, 7) || line(x, y, 15, 15, 22, 22),
    }
}

const fn prepare() -> [[u32; EDGE]; 25] {
    let mut rows = [[0; EDGE]; 25];
    let mut index = 0;
    while index < GraphicsSymbol::ALL.len() {
        let mut y = 0;
        while y < EDGE {
            let mut x = 0;
            while x < EDGE {
                if ink(GraphicsSymbol::ALL[index], x as i32, y as i32) {
                    rows[index][y] |= 1 << x;
                }
                x += 1;
            }
            y += 1;
        }
        index += 1;
    }
    rows
}

const RASTERS: [[u32; EDGE]; 25] = prepare();

pub(super) fn raster(symbol: GraphicsSymbol) -> &'static [u32; EDGE] {
    // ALL is exhaustive and its order is the resource table's only index.
    let index = GraphicsSymbol::ALL
        .iter()
        .position(|item| *item == symbol)
        .unwrap();
    &RASTERS[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_symbol_has_a_distinct_bounded_raster() {
        assert_eq!(core::mem::size_of_val(&RASTERS), 2400);
        for (index, rows) in RASTERS.iter().enumerate() {
            assert!(rows.iter().any(|row| *row != 0));
            assert!(rows.iter().all(|row| *row < 1 << EDGE));
            assert!(!RASTERS[..index].contains(rows));
        }
    }
}
