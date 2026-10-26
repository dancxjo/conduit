//! Make-checked meaning embedded in the trusted firmware. No Plan or Host/Boot
//! identity is embedded here; the ordinary planner must choose those freshly.
include!(concat!(env!("OUT_DIR"), "/network-identity.rs"));
mod material {
    include!(concat!(env!("OUT_DIR"), "/network-software.rs"));
}
pub fn load() -> conduit_plot::flat_image::indexed::IndexedSourceImage<'static> {
    use sha2::{Digest, Sha256};
    let image = include_bytes!(concat!(env!("OUT_DIR"), "/network-source.json"));
    let expected = include_bytes!(concat!(env!("OUT_DIR"), "/network-source.sha256"));
    let actual: [u8; 32] = Sha256::digest(image).into();
    assert_eq!(&actual, expected);
    // SAFETY: the independent expected digest and image are linked into this
    // trusted Make firmware. The build producer ran the shared ordinary checker
    // and expander; no AP/UART bytes or caller-supplied identity enter this path.
    unsafe {
        conduit_plot::flat_image::indexed::IndexedSourceImage::from_make_image(image).map(|view| {
            view.with_make_gear_decoder(material::decode_planning_gear)
                .with_make_configuration_decoder(material::decode_planning_configuration)
        })
    }
    .expect("checked Source image and exact linked constructors")
}
