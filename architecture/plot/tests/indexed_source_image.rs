#![cfg(feature = "flat-image")]
//! Representation/bounds proof, not authentication of arbitrary image bytes.
use conduit_plot::{
    flat_image::{indexed::*, ImageRefusal},
    *,
};
#[path = "common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
fn source() -> ExpandedCanonicalPlot {
    let syntax =
        parse_syntax_document("plot sample (\n >> value: U64\n result: U64 >>\n) = (. + .)\n");
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    expand_canonical_plot_for_authoring(&checked, "sample", &ProfileCatalog::new())
        .unwrap()
        .expanded
}
fn view(bytes: &[u8]) -> Result<IndexedSourceImage<'_>, ImageRefusal> {
    // SAFETY: only encoder-produced fixture bytes and their explicit malformed
    // variants below enter this test helper. It asserts no runtime trust.
    unsafe { IndexedSourceImage::from_make_image(bytes) }
}
#[test]
fn index_is_borrowed_and_exact_source_reconstruction_preserves_meaning() {
    let source = source();
    let bytes = encode(&source).unwrap();
    let (image, allocation) = allocation_probe::observe(|| view(&bytes).unwrap());
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
    assert_eq!(image.gear_count(), source.gears.len());
    assert_eq!(
        image.identity().unwrap(),
        image.metadata().unwrap().identity()
    );
    let metadata = image.metadata().unwrap();
    assert_eq!(metadata.identity().checked_plot_id, source.checked_plot_id);
    let gears = (0..image.gear_count())
        .map(|i| {
            let gear = image.gear(i).unwrap();
            assert_eq!(gear, source.gears[i]);
            gear
        })
        .collect();
    assert_eq!(metadata.into_expanded(gears).unwrap(), source);
    assert!(image.gear(usize::MAX).is_err());
    assert!(image.metadata().unwrap().into_expanded(Vec::new()).is_err());
}
#[test]
fn every_truncation_and_noncanonical_directory_refuses_before_decoding() {
    let bytes = encode(&source()).unwrap();
    for length in 0..bytes.len() {
        assert!(view(&bytes[..length]).is_err(), "cut {length}");
    }
    for (at, replacement) in [(0, b'?'), (8, 0), (10, 1), (16, 1)] {
        let mut bad = bytes.clone();
        bad[at] = replacement;
        assert!(view(&bad).is_err(), "index byte {at}");
    }
    for end in [0, u32::MAX] {
        let mut bad = bytes.clone();
        bad[20..24].copy_from_slice(&end.to_le_bytes());
        assert!(view(&bad).is_err());
    }
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(view(&trailing).is_err());
    assert!(matches!(
        view(&vec![0; MAXIMUM_BYTES + 1]),
        Err(ImageRefusal::Oversized)
    ));
}
#[test]
fn gear_decode_refusal_is_lazy_and_bounds_are_independent_of_metadata() {
    let mut bytes = encode(&source()).unwrap();
    let metadata_bytes = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let payload = 16 + 8 + metadata_bytes;
    bytes[payload] = b'?';
    let image = view(&bytes).unwrap();
    assert!(image.metadata().is_ok());
    assert!(image.gear(0).is_err());
}
#[test]
fn encode_refuses_empty_or_oversized_graphs_and_changed_provenance() {
    let original = source();
    let mut empty = original.clone();
    empty.gears.clear();
    assert_eq!(encode(&empty), Err(ImageRefusal::UnsupportedGraph));
    let mut many = original.clone();
    many.gears
        .resize(MAXIMUM_GEARS + 1, original.gears[0].clone());
    assert_eq!(encode(&many), Err(ImageRefusal::UnsupportedGraph));
    let mut changed = original;
    changed.provenance_digest.push('0');
    assert_eq!(encode(&changed), Err(ImageRefusal::Provenance));
}

#[test]
fn cord_visit_preserves_complete_records_without_retaining_provenance_or_vectors() {
    let source = source();
    let bytes = encode(&source).unwrap();
    let image = view(&bytes).unwrap();
    let mut expected = 0;
    let (count, memory) = allocation_probe::observe(|| {
        image
            .visit_connections(|cord| {
                assert_eq!(cord, &source.connections[expected]);
                expected += 1;
            })
            .unwrap()
    });
    assert_eq!(count, source.connections.len());
    assert_eq!(expected, count);
    assert_eq!(memory.retained_bytes(), 0);
    let (_, rich) = allocation_probe::observe(|| image.metadata().unwrap());
    assert!(memory.peak_bytes < rich.peak_bytes);
    let (_, projected) = allocation_probe::observe(|| image.planning_metadata().unwrap());
    assert!(memory.peak_bytes <= projected.peak_bytes);
}
