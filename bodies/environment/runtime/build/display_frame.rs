//! Derive byte locations by changing one checked scalar through the ordinary
//! composer. No wire offset or self-describing Type layout is handwritten.
#[path = "display_frame_reference.rs"]
mod reference;
pub fn emit(input: &conduit_core::StructuredInfoType, out: &std::path::Path) {
    let baseline = reference::encode(input, [0; 8]);
    assert!(baseline.len() <= 1024);
    let mut offsets = [0; 8];
    for n in 0..8 {
        let mut values = [0; 8];
        values[n] = 1;
        let changed = reference::encode(input, values);
        assert_eq!(baseline.len(), changed.len());
        let differences: Vec<_> = baseline
            .iter()
            .zip(&changed)
            .enumerate()
            .filter_map(|(n, (a, b))| (a != b).then_some(n))
            .collect();
        let [offset] = differences.as_slice() else {
            panic!("one scalar byte per Frame field")
        };
        assert_eq!((baseline[*offset], changed[*offset]), (0, 1));
        assert!(!offsets[..n].contains(offset));
        offsets[n] = *offset;
    }
    std::fs::write(
        out.join("tm1637-frame-material.rs"),
        format!(
            "pub const TEMPLATE: [u8; {}] = {:?};\npub const OFFSETS: [usize; 8] = {:?};\n",
            baseline.len(),
            baseline,
            offsets,
        ),
    )
    .unwrap();
}
