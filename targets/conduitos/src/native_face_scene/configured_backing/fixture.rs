//! Existing checked component Source over one semantic CPU Face and image.
use conduit_core::*;
use conduit_face::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub(super) fn face() -> Face {
    let mut face = crate::native_face_scene::tests::face(7);
    for (id, role, name) in [
        ("clock/scoped", "time/civil-observation", "Date and time"),
        ("weather/scoped", "weather/conditions", "Weather"),
        ("image/scoped", "media/image", "Photograph"),
    ] {
        face.subjects.push(FaceSubject {
            identity: id.into(),
            role: FaceRole::Semantic(kind_id(role)),
            name: name.into(),
        });
    }
    for (subject, name, value) in [
        ("clock/scoped", "local-date", "2026-11-17"),
        ("clock/scoped", "local-time", "15:42:30"),
        ("clock/scoped", "timezone", "America/Los_Angeles"),
        ("weather/scoped", "availability", "fresh"),
        ("weather/scoped", "location", "Seattle"),
        ("weather/scoped", "conditions", "Clear"),
    ] {
        face.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value: FacePropertyValue::Text(value.into()),
        });
    }
    let image = ImageResource::new(
        ImageColorProfile::Absent,
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
        },
        ImagePixelExtent::new(2, 2).unwrap(),
        ImageFormat::Rgba8,
        "image/frame".into(),
    )
    .unwrap()
    .into_structured()
    .unwrap();
    face.properties.push(FaceProperty {
        subject: "image/scoped".into(),
        name: "image".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                image.value_type().profile().unwrap().value_kind().clone(),
                MAX_FACE_VALUE_BYTES,
                alloc::vec![],
            )
            .unwrap(),
            bytes: image.canonical_bytes().unwrap(),
        },
    });
    rebuild(face)
}
pub(super) fn rebuild(face: Face) -> Face {
    Face::new_with_semantics(
        face.revision,
        face.basis,
        face.subjects,
        face.relationships,
        face.properties,
        face.text,
        face.actions,
        face.disclosures,
    )
    .unwrap()
}
pub(super) fn configuration(encounter: AmbientClockEncounter) -> ConfiguredMaskConfiguration {
    let provider = crate::product_bases::fixture_surface_provider();
    let offer = conduit_conduitos_mask_offer::native_layout_host_advertisement(
        &"host/configured-scene-cpu".into(),
        &"boot/configured-scene-cpu".into(),
        1,
        &provider.entry,
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    );
    let source =
        ambient_clock_mask_source("clock/scoped", "weather/scoped", "image/scoped").unwrap();
    conduit_conduitos_mask_offer::prepare_layout_component_document(
        &offer,
        &source,
        encounter.root(),
        conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard,
    )
    .unwrap()
    .1
}
pub(super) fn structure<'a>(
    configuration: &ConfiguredMaskConfiguration,
    face: &'a Face,
) -> PreparedComponentLayout<'a> {
    let decode = |v: &StructuredConfigurationValue| {
        StructuredInfoValue::from_canonical_bytes(v.canonical_value()).unwrap()
    };
    prepare_component_layout(
        decode(configuration.layout.as_ref().unwrap()),
        decode(&configuration.recipe),
        face,
    )
    .unwrap()
}
pub(super) fn look(configuration: &ConfiguredMaskConfiguration) -> AdmittedVisualLook {
    admit_visual_look(
        &configuration.requested_look().unwrap(),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap()
}
