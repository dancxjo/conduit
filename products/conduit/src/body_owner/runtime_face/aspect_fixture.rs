//! Minimal checked semantic photograph observation; no acquisition or display claim.
use super::*;
use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape};
use conduit_plot::rust_binding::{self, NativePrimitive, NativeRustBinding};
fn primitive<T: NativePrimitive>(ty: &StructuredInfoType, value: &T) -> StructuredInfoValue {
    if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
        StructuredInfoValue::nominal(ty.clone(), primitive(representation, value)).unwrap()
    } else {
        rust_binding::primitive_into_structured(ty.clone(), value).unwrap()
    }
}
fn record(
    ty: &StructuredInfoType,
    mut make: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record");
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| StructuredFieldValue::new(f.name(), make(f.name(), f.value_type())).unwrap())
            .collect(),
    )
    .unwrap()
}
pub(super) fn input() -> ExternalForeInput {
    use conduit_core::*;
    use conduit_face::{ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource};
    let types = conduit_semantic_catalog::prepare_ambient_display_types().unwrap();
    let named = |name| types.native_types.iter().find(|t| t.name == name).unwrap();
    let image = ImageResource::new(
        ImageColorProfile::absent(),
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: kind_id("media/image-rgba8@1"),
            access_class: "conduit.resource/image-content@1".into(),
            extent: ResourceExtent {
                bytes: 16,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        },
        ImagePixelExtent::new(2, 2).unwrap(),
        ImageFormat::rgba8(),
        "fixture/photo".into(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    let metadata = record(&named("AmbientWallpaperImage").value_type, |name, ty| {
        if name == "image" {
            return image.clone();
        }
        primitive(
            ty,
            &match name {
                "title" => "Fixture mountain light",
                "caption" => "Mountain sunrise",
                "attribution" => "Fixture photographer credit",
                "license" => "Fixture license",
                "source" => "Projection fixture",
                _ => panic!("metadata"),
            }
            .to_owned(),
        )
    });
    let selection = record(
        &named("AmbientWallpaperSelection").value_type,
        |name, ty| match name {
            "image" => metadata.clone(),
            "selected_at_unix_ms" => primitive(ty, &1000u64),
            "next_rotation_unix_ms" => primitive(ty, &301000u64),
            "rotation_seconds" => primitive(ty, &300u64),
            _ => panic!("selection"),
        },
    );
    let wallpaper = StructuredInfoValue::variant(
        named("AmbientWallpaperObservation").value_type.clone(),
        "available",
        selection,
    )
    .unwrap();
    let weather =
        conduit_std_host::ambient_weather::PreparedOpenMeteoWeather::prepare("Seattle", 900000)
            .unwrap()
            .unavailable("Fixture weather unavailable")
            .unwrap();
    let clock = conduit_std_host::ambient_clock::PreparedAmbientClockWeather::prepare(
        "America/Los_Angeles",
    )
    .unwrap()
    .snapshot(150000, &weather)
    .unwrap();
    let home = named("AmbientHomeObservation");
    let observation = record(&home.value_type, |name, _| match name {
        "wallpaper" => wallpaper.clone(),
        "clock_weather" => clock.clone(),
        _ => panic!("observation"),
    });
    rust_binding::validate_native_contracts(&observation, &home.value_contracts).unwrap();
    rust_binding::validate_native_invariants(&observation, &home.invariants).unwrap();
    ExternalForeInput {
        front_port_id: "observations".into(),
        track: ConnectionTrack::Payload,
        bytes: observation.canonical_bytes().unwrap(),
    }
}
