use alloc::vec::Vec;
use super::*;
use conduit_core::{
    kind_id, BoundedResourceRef, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape,
};
use conduit_plot::rust_binding::{primitive_into_structured, NativePrimitive};

pub(super) fn primitive<T: NativePrimitive>(
    ty: &StructuredInfoType,
    value: T,
) -> StructuredInfoValue {
    fn make<T: NativePrimitive>(ty: &StructuredInfoType, value: &T) -> StructuredInfoValue {
        if let StructuredInfoTypeShape::Nominal { representation, .. } = ty.shape() {
            StructuredInfoValue::nominal(ty.clone(), make(representation, value)).unwrap()
        } else {
            primitive_into_structured(ty.clone(), value).unwrap()
        }
    }
    make(ty, &value)
}

pub(super) fn record(
    ty: &StructuredInfoType,
    mut value: impl FnMut(&str, &StructuredInfoType) -> StructuredInfoValue,
) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|field| {
                StructuredFieldValue::new(field.name(), value(field.name(), field.value_type()))
                    .unwrap()
            })
            .collect(),
    )
    .unwrap()
}

fn case(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload_type = cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type();
    StructuredInfoValue::variant(ty.clone(), tag, primitive(payload_type, ())).unwrap()
}

pub(super) fn resource() -> Vec<u8> {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([1; 32]),
        content_profile: kind_id("media/image-rgba8@1"),
        access_class: ResourceClassId::from("conduit.resource/image-content@1"),
        extent: ResourceExtent {
            bytes: 16,
            items: Some(1),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([2; 32]),
            expires_at: None,
        },
    }
    .encode()
    .unwrap()
}

pub(super) fn observed_wallpaper(
    prepared: &PreparedAmbientFaces,
    state: &str,
    next: u64,
) -> StructuredInfoValue {
    let ty = &prepared.wallpaper_observation_type().value_type;
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("wallpaper state")
    };
    let payload_type = cases
        .iter()
        .find(|case| case.tag() == state)
        .unwrap()
        .payload_type();
    let payload = match state {
        "available" => wallpaper(prepared, next),
        "stale" => record(payload_type, |name, ty| match name {
            "selection" => wallpaper(prepared, next),
            "reason" => primitive(ty, String::from("Acquisition failed")),
            _ => panic!("stale wallpaper field"),
        }),
        "unavailable" => primitive(payload_type, String::from("Acquisition failed")),
        _ => panic!("wallpaper state"),
    };
    StructuredInfoValue::variant(ty.clone(), state, payload).unwrap()
}

pub(super) fn wallpaper(prepared: &PreparedAmbientFaces, next: u64) -> StructuredInfoValue {
    record(
        &prepared.wallpaper_type().value_type,
        |name, ty| match name {
            "selected_at_unix_ms" => primitive(ty, 1_000_u64),
            "next_rotation_unix_ms" => primitive(ty, next),
            "rotation_seconds" => primitive(ty, 300_u64),
            "image" => record(ty, |name, ty| match name {
                "title" => primitive(ty, String::from("Mountain light")),
                "caption" => primitive(ty, String::from("Sunrise over the Cascades")),
                "attribution" => primitive(ty, String::from("Example photographer")),
                "license" => primitive(ty, String::from("CC BY-SA 4.0")),
                "source" => primitive(ty, String::from("Example collection")),
                "image" => record(ty, |name, ty| match name {
                    "content" => StructuredInfoValue::leaf(ty.clone(), resource()).unwrap(),
                    "color_profile" => case(ty, "absent"),
                    "format" => case(ty, "rgba8"),
                    "extent" => record(ty, |_, ty| primitive(ty, 2_u64)),
                    "image_frame" => primitive(ty, String::from("photo/one")),
                    _ => panic!("image field"),
                }),
                _ => panic!("metadata field"),
            }),
            _ => panic!("wallpaper field"),
        },
    )
}

pub(super) fn clock(
    prepared: &PreparedAmbientFaces,
    availability: &str,
    snapshot: u64,
    reference: u64,
) -> StructuredInfoValue {
    record(
        &prepared.clock_weather_type().value_type,
        |name, ty| match name {
            "local_date" => primitive(ty, String::from("2026-11-15")),
            "local_time" => primitive(ty, String::from("21:30:15")),
            "timezone" => primitive(ty, String::from("America/Los_Angeles")),
            "timezone_rule_set" => primitive(ty, String::from("iana-test/rules")),
            "utc_offset_minutes" => primitive(ty, -480_i64),
            "observed_at_unix_ms" => primitive(ty, snapshot),
            "weather" => {
                let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
                    panic!("weather variant")
                };
                let payload_type = cases
                    .iter()
                    .find(|case| case.tag() == availability)
                    .unwrap()
                    .payload_type();
                let payload = if availability == "unavailable" {
                    primitive(payload_type, String::from("Provider could not be reached"))
                } else {
                    record(payload_type, |name, ty| match name {
                        "reference_at_unix_ms" => primitive(ty, reference),
                        "reading" => record(ty, |name, ty| match name {
                            "location" => primitive(ty, String::from("Seattle")),
                            "attribution" => primitive(ty, String::from("Open-Meteo")),
                            "conditions" => primitive(ty, String::from("Light rain")),
                            "temperature_celsius_milli" => primitive(ty, -2_125_i64),
                            "observed_at_unix_ms" => primitive(ty, 100_u64),
                            "usable_until_unix_ms" => primitive(ty, 200_u64),
                            _ => panic!("weather reading field"),
                        }),
                        _ => panic!("weather state field"),
                    })
                };
                StructuredInfoValue::variant(ty.clone(), availability, payload).unwrap()
            }
            _ => panic!("clock field"),
        },
    )
}

pub(super) fn basis() -> FaceContributionBasis {
    FaceContributionBasis {
        checked_plot_id: "plot/ambient".into(),
        plan_id: "plan/ambient".into(),
        active_play_id: "play/ambient".into(),
        required_interaction_context: None,
    }
}

pub(super) fn face(fragment: FaceFragment) -> conduit_face::Face {
    conduit_face::Face::new_with_semantics(
        1,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/ambient".into()),
            checked_plot_id: Some(fragment.basis.checked_plot_id),
            expanded_plot_id: Some("expanded/ambient".into()),
            plan_id: Some(fragment.basis.plan_id),
            active_play_id: Some(fragment.basis.active_play_id),
            sign_ids: Vec::new(),
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
        fragment.actions,
        fragment.disclosures,
    )
    .and_then(|face| face.with_composition(fragment.composition))
    .unwrap()
}
