use super::*;
use crate::ambient_wallpaper::{MAXIMUM_APOD_RESPONSE_BYTES, acquisition::*};
use image::{ExtendedColorType, ImageEncoder, codecs::png::PngEncoder};
fn deck() -> PreparedDeck {
    let admission = PreparationAdmission::admit(
        chrono::NaiveDate::from_ymd_opt(2026, 11, 16).unwrap(),
        MAXIMUM_RETAINED_BYTES,
        PREPARATION_BYTES,
    )
    .unwrap();
    let mut number = 0;
    prepare_deck(admission, |url, bound| {
        if bound == MAXIMUM_APOD_RESPONSE_BYTES {
            Ok(serde_json::to_vec(&serde_json::json!({"date":url.rsplit('=').next().unwrap(),"title":"Actual title","media_type":"image","hdurl":"https://apod.nasa.gov/fixture.png"})).unwrap())
        } else {
            number += 1;
            let mut bytes = Vec::new();
            PngEncoder::new(&mut bytes).write_image(&[number,2,3,255],1,1,ExtendedColorType::Rgba8).unwrap();
            Ok(bytes)
        }
    }).unwrap()
}
fn field<'a>(v: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let conduit_core::StructuredInfoValueShape::Record(fields) = v.shape() else {
        panic!("record");
    };
    fields.iter().find(|f| f.name() == name).unwrap().value()
}
fn selection(v: &StructuredInfoValue) -> &StructuredInfoValue {
    let conduit_core::StructuredInfoValueShape::Variant { tag, payload } = v.shape() else {
        panic!("variant");
    };
    assert_eq!(tag, "available");
    payload
}
fn text(v: &StructuredInfoValue) -> String {
    rust_binding::primitive_from_structured(v).unwrap()
}
fn unsigned(v: &StructuredInfoValue) -> u64 {
    rust_binding::primitive_from_structured(v).unwrap()
}
#[test]
fn actual_four_generations_rotate_only_on_selection_and_match_checked_domain() {
    let deck = deck();
    let mut wire = PreparedWallpaperSelectionWire::prepare(&deck).unwrap();
    let types = conduit_semantic_catalog::prepare_ambient_display_types().unwrap();
    let ty = types
        .native_types
        .iter()
        .find(|t| t.name == "AmbientWallpaperObservation")
        .unwrap();
    let rotation_type = &types
        .native_types
        .iter()
        .find(|ty| ty.name == "AmbientRotationSeconds")
        .unwrap()
        .value_type;
    assert!(matches!(
        rotation_type.shape(),
        StructuredInfoTypeShape::Nominal { .. }
    ));
    let first = wire.encode(1000).unwrap().to_vec();
    assert_eq!(wire.encode(300999).unwrap(), first);
    assert_eq!(wire.encode(1000), Err(SelectionRefusal::BackwardsTime));
    let mut wire = PreparedWallpaperSelectionWire::prepare(&deck).unwrap();
    for (index, now) in [1000, 301000, 601000, 901000, 1201000]
        .into_iter()
        .enumerate()
    {
        let value = StructuredInfoValue::from_canonical_bytes(wire.encode(now).unwrap()).unwrap();
        assert_eq!(value.value_type(), &ty.value_type);
        rust_binding::validate_native_contracts(&value, &ty.value_contracts).unwrap();
        rust_binding::validate_native_invariants(&value, &ty.invariants).unwrap();
        assert_eq!(value.canonical_bytes().unwrap(), wire.encode(now).unwrap());
        let selected = selection(&value);
        assert_eq!(unsigned(field(selected, "selected_at_unix_ms")), now);
        assert_eq!(
            unsigned(field(selected, "next_rotation_unix_ms")),
            now + 300000
        );
        let rotation = field(selected, "rotation_seconds");
        assert_eq!(rotation.value_type(), rotation_type);
        assert_eq!(unsigned(rotation), 300);
        let metadata = field(selected, "image");
        assert_eq!(text(field(metadata, "title")), "Actual title");
        assert_eq!(text(field(metadata, "caption")), "");
        assert_eq!(
            text(field(metadata, "attribution")),
            "Creator credit not supplied by NASA APOD"
        );
        assert_eq!(
            text(field(metadata, "license")),
            "License not supplied by provider"
        );
        let photo = &deck.photos()[index % 4];
        assert_eq!(
            text(field(metadata, "source")),
            format!("NASA APOD/{}", photo.candidate().date)
        );
        let image = ImageResource::from_structured(field(metadata, "image").clone()).unwrap();
        assert_eq!(image.content().identity.digest(), *photo.original_sha256());
        assert_eq!(
            image.content().lifetime.version.digest(),
            *photo.version_sha256()
        );
        assert_eq!(
            image.content().extent,
            ResourceExtent {
                bytes: 4,
                items: Some(1)
            }
        );
        assert_eq!(image.format(), &ImageFormat::rgba8());
        assert_eq!(image.extent().width(), &1);
        assert_eq!(image.extent().height(), &1);
    }
}
#[test]
fn encode_errors_and_rotations_have_no_heap_growth() {
    let deck = deck();
    for interval in [0, 29, 3601, u64::MAX] {
        assert!(matches!(
            PreparedWallpaperSelectionWire::prepare_with_interval(&deck, interval),
            Err(SelectionRefusal::InvalidInterval)
        ));
    }
    for interval in [30, 300, 3600] {
        let mut wire =
            PreparedWallpaperSelectionWire::prepare_with_interval(&deck, interval).unwrap();
        let heap = wire.prepared_heap_bytes();
        let probe = crate::allocation_probe::begin();
        assert_eq!(
            wire.encode(u64::MAX),
            Err(SelectionRefusal::TimestampOverflow)
        );
        wire.encode(1000).unwrap();
        wire.encode(1000 + interval * 1000 - 1).unwrap();
        wire.encode(1000 + interval * 1000).unwrap();
        assert_eq!(wire.encode(0), Err(SelectionRefusal::BackwardsTime));
        for n in 2..20 {
            wire.encode(1000 + n * interval * 1000).unwrap();
        }
        assert_eq!(probe.finish_statistics().requested_bytes, 0);
        assert_eq!(wire.prepared_heap_bytes(), heap);
    }
}
