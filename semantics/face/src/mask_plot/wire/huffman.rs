//! Complete fixed parsing profile and bounded derived output trees.
//! No document is retained. Canonical codewords are already bit-reversed for
//! RFC1951 packing; the fixed profile represents every byte and length.
//! Fixed weights were selected from an ordinary Mask metadata fixture;
//! they affect compression only, never the decoded Plan or its admission.
pub(super) static LENGTHS: [u8; 286] = [
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 10, 11, 7, 11, 11, 11, 11, 11, 11, 11, 11, 11, 8, 9, 11, 9, 5,
    5, 5, 6, 5, 6, 6, 5, 5, 6, 8, 11, 11, 11, 11, 11, 9, 10, 11, 9, 10, 11, 9, 11, 11, 11, 11, 11,
    9, 11, 11, 11, 10, 11, 9, 11, 10, 11, 11, 11, 11, 11, 11, 9, 11, 9, 11, 7, 11, 5, 6, 5, 5, 4,
    6, 7, 8, 6, 11, 8, 6, 7, 6, 6, 7, 10, 6, 6, 6, 6, 8, 10, 9, 9, 11, 8, 11, 7, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11, 11,
    11, 11, 11, 11, 11, 11, 11, 4, 5, 6, 6, 7, 6, 8, 8, 6, 7, 8, 8, 8, 8, 9, 9, 9, 8, 9, 8, 6, 8,
    8, 11, 10, 10, 10, 10, 10,
];
pub(super) static CODES: [u16; 286] = [
    359, 1383, 871, 1895, 231, 1255, 743, 1767, 487, 1511, 999, 2023, 23, 1047, 535, 1559, 279,
    1303, 791, 1815, 151, 1175, 663, 1687, 407, 1431, 919, 1943, 87, 1111, 599, 1623, 455, 343, 61,
    1367, 855, 1879, 215, 1239, 727, 1751, 471, 1495, 51, 59, 983, 315, 4, 20, 12, 14, 28, 46, 30,
    2, 18, 62, 179, 2007, 55, 1079, 567, 1591, 187, 967, 311, 443, 39, 1335, 123, 823, 1847, 183,
    1207, 695, 379, 1719, 439, 1463, 551, 951, 251, 1975, 295, 119, 1143, 631, 1655, 375, 1399,
    507, 887, 7, 1911, 125, 247, 10, 1, 26, 6, 0, 33, 3, 115, 17, 1271, 243, 49, 67, 9, 41, 35,
    807, 25, 57, 5, 37, 11, 167, 263, 135, 759, 139, 1783, 99, 503, 1527, 1015, 2039, 15, 1039,
    527, 1551, 271, 1295, 783, 1807, 143, 1167, 655, 1679, 399, 1423, 911, 1935, 79, 1103, 591,
    1615, 335, 1359, 847, 1871, 207, 1231, 719, 1743, 463, 1487, 975, 1999, 47, 1071, 559, 1583,
    303, 1327, 815, 1839, 175, 1199, 687, 1711, 431, 1455, 943, 1967, 111, 1135, 623, 1647, 367,
    1391, 879, 1903, 239, 1263, 751, 1775, 495, 1519, 1007, 2031, 31, 1055, 543, 1567, 287, 1311,
    799, 1823, 159, 1183, 671, 1695, 415, 1439, 927, 1951, 95, 1119, 607, 1631, 351, 1375, 863,
    1887, 223, 1247, 735, 1759, 479, 1503, 991, 2015, 63, 1087, 575, 1599, 319, 1343, 831, 1855,
    191, 1215, 703, 1727, 447, 1471, 959, 1983, 127, 1151, 639, 1663, 383, 1407, 895, 1919, 255,
    1279, 767, 1791, 511, 1535, 1023, 8, 22, 21, 53, 19, 13, 75, 203, 45, 83, 43, 171, 107, 235,
    391, 71, 327, 27, 199, 155, 29, 91, 219, 2047, 679, 423, 935, 103, 615,
];

/// Bounded complete tree; rare pathological histograms retain the complete
/// predefined tree rather than emitting an invalid overlong code.
pub(super) struct Tree {
    pub(super) lengths: [u8; 286],
    pub(super) codes: [u16; 286],
    pub(super) distance_lengths: [u8; 30],
    pub(super) distance_codes: [u16; 30],
}
impl Tree {
    pub(super) fn from_histogram(histogram: &[u32]) -> alloc::boxed::Box<Self> {
        let mut tree = alloc::boxed::Box::new(Self {
            lengths: LENGTHS,
            codes: CODES,
            distance_lengths: core::array::from_fn(|i| if i < 2 { 4 } else { 5 }),
            distance_codes: core::array::from_fn(|i| {
                let bits = if i < 2 { 4 } else { 5 };
                let code = if i < 2 { i } else { i + 2 };
                (code as u16).reverse_bits() >> (16 - bits)
            }),
        });
        derive(&histogram[..286], &mut tree.lengths, &mut tree.codes, 15);
        if histogram.len() == 316 {
            derive(
                &histogram[286..],
                &mut tree.distance_lengths,
                &mut tree.distance_codes,
                15,
            );
        }
        tree
    }
}
struct Weights([alloc::boxed::Box<[u32]>; 3]);
impl core::ops::Index<usize> for Weights {
    type Output = u32;
    fn index(&self, index: usize) -> &u32 {
        &self.0[index / 256][index % 256]
    }
}
impl core::ops::IndexMut<usize> for Weights {
    fn index_mut(&mut self, index: usize) -> &mut u32 {
        &mut self.0[index / 256][index % 256]
    }
}
pub(super) fn derive(histogram: &[u32], lengths: &mut [u8], codes: &mut [u16], maximum: u8) {
    use alloc::vec;
    let symbols = histogram.len();
    let nodes = symbols * 2 - 1;
    let mut weights = Weights(core::array::from_fn(|_| {
        vec![u32::MAX; 256].into_boxed_slice()
    }));
    let mut parents = vec![u16::MAX; nodes].into_boxed_slice();
    for (index, &count) in histogram.iter().enumerate() {
        weights[index] = if count == 0 { u32::MAX } else { count.max(1) };
    }
    let active = (0..symbols)
        .filter(|&index| weights[index] != u32::MAX)
        .count();
    if active < 2 {
        return;
    }
    for next in symbols..symbols + active - 1 {
        let mut selected = [usize::MAX; 2];
        for index in 0..next {
            if parents[index] != u16::MAX || weights[index] == u32::MAX {
                continue;
            }
            if selected[0] == usize::MAX
                || (weights[index], index) < (weights[selected[0]], selected[0])
            {
                selected[1] = selected[0];
                selected[0] = index;
            } else if selected[1] == usize::MAX
                || (weights[index], index) < (weights[selected[1]], selected[1])
            {
                selected[1] = index;
            }
        }
        weights[next] = weights[selected[0]] + weights[selected[1]];
        parents[selected[0]] = next as u16;
        parents[selected[1]] = next as u16;
    }
    let mut derived = vec![0u8; symbols];
    let mut counts = [0u16; 16];
    for (symbol, length) in derived.iter_mut().enumerate() {
        if weights[symbol] == u32::MAX {
            continue;
        }
        let (mut depth, mut node) = (0u8, symbol);
        while parents[node] != u16::MAX {
            depth += 1;
            node = usize::from(parents[node]);
        }
        if depth > maximum {
            return;
        }
        *length = depth;
        counts[usize::from(depth)] += 1;
    }
    lengths.copy_from_slice(&derived);
    let mut next = [0u16; 16];
    let mut code = 0;
    for bits in 1..16 {
        code = (code + counts[bits - 1]) << 1;
        next[bits] = code;
    }
    for symbol in 0..symbols {
        let bits = usize::from(lengths[symbol]);
        if bits == 0 {
            codes[symbol] = 0;
            continue;
        }
        codes[symbol] = next[bits].reverse_bits() >> (16 - bits);
        next[bits] += 1;
    }
}
