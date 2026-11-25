use crate::{
    test_fixtures::{observed_wallpaper, wallpaper},
    PreparedAmbientFaces, PreparedWallpaperSelection, SelectionRefusal,
};
use conduit_plot::rust_binding::semantic_core::StructuredInfoValueShape;

#[test]
fn checked_wallpaper_metadata_retains_the_exact_observation_until_rotation() {
    let prepared = PreparedAmbientFaces::prepare().unwrap();
    let selection = wallpaper(&prepared, 301_000);
    let StructuredInfoValueShape::Record(fields) = selection.shape() else {
        panic!("wallpaper selection record");
    };
    let metadata = fields
        .iter()
        .find(|field| field.name() == "image")
        .unwrap()
        .value();
    let mut wire =
        PreparedWallpaperSelection::prepare(core::array::from_fn(|_| metadata.clone()), 300)
            .unwrap();
    let expected = observed_wallpaper(&prepared, "available", 301_000)
        .canonical_bytes()
        .unwrap();
    assert_eq!(wire.encode(1_000).unwrap(), expected);
    assert_eq!(wire.encode(300_999).unwrap(), expected);
    assert_eq!(wire.encode(300_998), Err(SelectionRefusal::BackwardsTime));
    assert_ne!(wire.encode(301_000).unwrap(), expected);
}
