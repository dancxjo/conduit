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
fn exact_gear_identity_does_not_materialize_configuration_or_contracts() {
    let source = source();
    let bytes = encode(&source).unwrap();
    let image = view(&bytes).unwrap();
    for index in 0..image.gear_count() {
        let (identity, thin) = allocation_probe::observe(|| image.gear_identity(index).unwrap());
        let (complete, rich) = allocation_probe::observe(|| image.gear(index).unwrap());
        assert_eq!(
            identity,
            (complete.gear_id.clone(), complete.kind_id.clone())
        );
        assert!(
            thin.peak_bytes < rich.peak_bytes,
            "{thin:?} versus {rich:?}"
        );
    }
    assert!(image.gear_identity(image.gear_count()).is_err());
    assert!(image.gear_identity(usize::MAX).is_err());
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
    assert!(image.gear_identity(0).is_err());
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

fn linked_gear(index: usize) -> Result<CheckedGear, ImageRefusal> {
    source()
        .gears
        .get(index)
        .cloned()
        .ok_or(ImageRefusal::Representation)
}
fn linked_configuration(index: usize) -> Result<IndexedGearConfiguration, ImageRefusal> {
    let gear = linked_gear(index)?;
    Ok(IndexedGearConfiguration {
        gear_id: gear.gear_id,
        configuration: gear.configuration,
    })
}
fn linked_view(bytes: &[u8]) -> Result<IndexedSourceImage<'_>, ImageRefusal> {
    // SAFETY: these callbacks reconstruct the exact same fixture Source encoded
    // by this test. This is representation proof, not authentication of ingress.
    unsafe {
        IndexedSourceImage::from_make_image_with_decoders(bytes, linked_gear, linked_configuration)
    }
}
#[test]
fn linked_make_constructor_preserves_complete_gears_configuration_and_bounds() {
    let bytes = encode(&source()).unwrap();
    let ordinary = view(&bytes).unwrap();
    let (linked, memory) = allocation_probe::observe(|| linked_view(&bytes).unwrap());
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(ordinary.identity().unwrap(), linked.identity().unwrap());
    assert_eq!(ordinary.gear_count(), linked.gear_count());
    for index in 0..ordinary.gear_count() {
        assert_eq!(ordinary.gear(index).unwrap(), linked.gear(index).unwrap());
        let expected = ordinary.gear_configuration(index).unwrap();
        let actual = linked.gear_configuration(index).unwrap();
        assert_eq!(expected.gear_id, actual.gear_id);
        assert_eq!(expected.configuration, actual.configuration);
    }
    assert!(linked.gear(ordinary.gear_count()).is_err());
    assert!(linked.gear(usize::MAX).is_err());
    assert!(linked.gear_configuration(ordinary.gear_count()).is_err());
    assert!(linked.gear_configuration(usize::MAX).is_err());
    for end in 0..bytes.len() {
        assert!(linked_view(&bytes[..end]).is_err(), "cut {end}");
    }
}
#[test]
fn linked_driver_does_not_decode_payload_as_json() {
    let mut bytes = encode(&source()).unwrap();
    let metadata_length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let payload = 16 + (source().gears.len() + 1) * 4 + metadata_length;
    bytes[payload] = b'?';
    assert!(view(&bytes).unwrap().gear(0).is_err());
    // Explicitly damaged local fixture bytes exercise driver selection only;
    // these bytes are not authenticated and confer no Source/admission trust.
    assert_eq!(
        linked_view(&bytes).unwrap().gear(0).unwrap(),
        linked_gear(0).unwrap()
    );
}
fn with_metadata_gears(bytes: &[u8], gears: &str) -> Vec<u8> {
    let count = usize::from(u16::from_le_bytes(bytes[8..10].try_into().unwrap()));
    let start = 16 + (count + 1) * 4;
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let metadata = core::str::from_utf8(&bytes[start..start + length]).unwrap();
    let replacement = metadata.replacen("\"gears\":[]", &format!("\"gears\":{gears}"), 1);
    assert_ne!(replacement, metadata);
    let mut altered = bytes[..start].to_vec();
    altered[12..16].copy_from_slice(&(replacement.len() as u32).to_le_bytes());
    altered.extend_from_slice(replacement.as_bytes());
    altered.extend_from_slice(&bytes[start + length..]);
    altered
}
#[test]
fn planning_metadata_refuses_first_gear_without_decoding_nested_contracts() {
    let source = source();
    let bytes = encode(&source).unwrap();
    let nested = format!("[{{\"kind_id\":\"{}\"}}]", "x".repeat(12_000));
    let altered = with_metadata_gears(&bytes, &nested);
    let image = view(&altered).unwrap();
    let (result, memory) = allocation_probe::observe(|| image.planning_metadata());
    assert!(result.is_err());
    assert!(memory.peak_bytes < 1024, "{memory:?}");
    let complete = serde_json::to_string(&source.gears[..1]).unwrap();
    for nonempty in [&complete[..], "[null]", "[1]", "{}", "null"] {
        let altered = with_metadata_gears(&bytes, nonempty);
        assert!(view(&altered).unwrap().planning_metadata().is_err());
    }
    assert!(view(&bytes).unwrap().planning_metadata().is_ok());
}
