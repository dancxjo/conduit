//! Make-checked meaning embedded in the trusted firmware. No Plan or Host/Boot
//! identity is embedded here; the ordinary planner must choose those freshly.
pub fn load() -> conduit_plot::ExpandedCanonicalPlot {
    use sha2::{Digest, Sha256};
    let image = include_bytes!(concat!(env!("OUT_DIR"), "/network-source.json"));
    let expected = include_bytes!(concat!(env!("OUT_DIR"), "/network-source.sha256"));
    let actual: [u8; 32] = Sha256::digest(image).into();
    assert_eq!(&actual, expected);
    // SAFETY: the independent expected digest and image are linked into this
    // trusted Make firmware. The build producer ran the shared ordinary checker
    // and expander; no AP/UART bytes or caller-supplied identity enter this path.
    unsafe { conduit_plot::flat_image::decode_trusted(image) }.expect("checked Source image")
}
