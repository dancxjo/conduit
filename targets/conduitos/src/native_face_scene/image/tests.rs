use super::*;
use conduit_core::*;
use conduit_plot::rust_binding::primitive_into_structured;

fn record(
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
            .map(|f| StructuredFieldValue::new(f.name(), value(f.name(), f.value_type())).unwrap())
            .collect(),
    )
    .unwrap()
}
fn case(ty: &StructuredInfoType, tag: &str) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|c| c.tag() == tag)
        .unwrap()
        .payload_type();
    StructuredInfoValue::variant(
        ty.clone(),
        tag,
        primitive_into_structured(payload.clone(), &()).unwrap(),
    )
    .unwrap()
}
fn image_face() -> Face {
    let resource = BoundedResourceRef {
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
    .unwrap();
    let ty = ImageResource::semantic_type().unwrap();
    let value = record(&ty, |name, ty| match name {
        "content" => StructuredInfoValue::leaf(ty.clone(), resource.clone()).unwrap(),
        "color_profile" => case(ty, "absent"),
        "format" => case(ty, "rgba8"),
        "extent" => record(ty, |_, ty| {
            primitive_into_structured(ty.clone(), &2_u64).unwrap()
        }),
        "image_frame" => {
            primitive_into_structured(ty.clone(), &alloc::string::String::from("photo/one"))
                .unwrap()
        }
        _ => panic!("field"),
    });
    let mut face = super::super::tests::face(1);
    face.properties.push(conduit_face::FaceProperty {
        subject: "z-first".into(),
        name: "arbitrary-image-property".into(),
        value: FacePropertyValue::TypedValue {
            contract: CheckedValueContract::new(
                ty.profile().unwrap().value_kind().clone(),
                conduit_face::MAX_FACE_VALUE_BYTES,
                alloc::vec![],
            )
            .unwrap(),
            bytes: value.canonical_bytes().unwrap(),
        },
    });
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
#[test]
fn ordinary_mask_projects_exact_typed_image_without_changing_semantics() {
    let face = image_face();
    let scene = NativeFaceScene::prepare(face.clone(), 640, 480).unwrap();
    assert_eq!(scene.face(), &face);
    let frame = scene.frame().unwrap();
    let images: alloc::vec::Vec<_> = frame
        .scene
        .commands()
        .iter()
        .filter_map(|c| c.image_key())
        .collect();
    assert_eq!(images, [GraphicsImageKey::new([1; 32], [2; 32]).unwrap()]);
    let static_scene = PreparedStaticNativeFaceScene::prepare(face, 640, 480).unwrap();
    let static_frame = static_scene.frame().unwrap();
    let static_images: alloc::vec::Vec<_> = static_frame
        .scene
        .commands()
        .iter()
        .filter_map(|c| c.image_key())
        .collect();
    assert_eq!(static_images, images);
}
#[test]
fn ordinary_face_without_typed_image_has_no_image_operation() {
    let scene = NativeFaceScene::prepare(super::super::tests::face(1), 640, 480).unwrap();
    assert!(
        scene
            .frame()
            .unwrap()
            .scene
            .commands()
            .iter()
            .all(|c| c.image_key().is_none())
    );
}
