use conduit_form::rust_binding::{generate_rust_bindings, RustBindingOptions};
use std::fs;
use std::process::Command;

#[test]
fn generated_reference_bindings_enforce_exact_leaf_contracts() {
    let source = "type Image = Bytes\ntype References = {\n    text: &Text\n    image: &Image\n    resource: ResourceRef\n}\n";
    let mut catalog = conduit_form::StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let checked =
        conduit_form::check_syntax_document(&conduit_form::parse_syntax_document(source), &catalog)
            .unwrap();
    let generated =
        generate_rust_bindings(&checked.native_types, &RustBindingOptions::default()).unwrap();
    let references = checked
        .native_types
        .iter()
        .find(|value_type| value_type.name == "References")
        .unwrap();
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } =
        references.value_type.shape()
    else {
        panic!("reference fixture must remain a record")
    };
    for name in ["text", "image"] {
        let field = fields.iter().find(|field| field.name() == name).unwrap();
        let conduit_core::StructuredInfoTypeShape::Leaf(kind) = field.value_type().shape() else {
            panic!("generation reference must remain a leaf")
        };
        assert!(kind.as_str().starts_with("data/generation-reference<"));
        assert!(kind.as_str().ends_with('>'));
    }
    let resource = fields
        .iter()
        .find(|field| field.name() == "resource")
        .unwrap();
    assert!(matches!(
        resource.value_type().shape(),
        conduit_core::StructuredInfoTypeShape::Leaf(kind)
            if kind.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID
    ));

    let directory = std::env::temp_dir().join(format!(
        "conduit-native-reference-bindings-{}",
        std::process::id()
    ));
    fs::create_dir_all(directory.join("src")).unwrap();
    let generated_source = directory.join("src/lib.rs");
    fs::write(
        &generated_source,
        format!("{}{}", generated.source, EXERCISE),
    )
    .unwrap();
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    fs::write(
        directory.join("Cargo.toml"),
        format!(
            "[package]\nname = \"generated-reference-proof\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nconduit-form = {{ path = {:?} }}\nconduit-data = {{ path = {:?} }}\n",
            repository.join("architecture/form"),
            repository.join("semantics/data")
        ),
    )
    .unwrap();
    let execution = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["test", "--offline", "--quiet", "--manifest-path"])
        .arg(directory.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", directory.join("target"))
        .output()
        .unwrap();
    assert!(
        execution.status.success(),
        "generated Rust reference proof failed:\n{}\n{}",
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    fs::remove_dir_all(directory).unwrap();
}

const EXERCISE: &str = r#"
#[cfg(test)]
mod reference_proof {
    use super::*;
    use conduit_core::{
        data_reference_kind, BoundedResourceRef, KindId, ResourceClassId, ResourceExtent,
        ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoTypeShape,
        StructuredInfoValue, TemporalInstant, TemporalScale, RESOURCE_REFERENCE_INFO_ID,
    };
    use conduit_data::{DataReference, DATA_GENERATION_ACCESS_CLASS};

    fn raw_reference(content: &str) -> BoundedResourceRef {
        BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([1; 32]),
            content_profile: KindId::from(content),
            access_class: ResourceClassId::from(DATA_GENERATION_ACCESS_CLASS),
            extent: ResourceExtent { bytes: 64, items: None },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([2; 32]),
                expires_at: None,
            },
        }
    }

    fn data_reference(content: &str) -> DataReference {
        DataReference::new(raw_reference(content)).unwrap()
    }

    fn generation_content_kind(field_name: &str) -> String {
        let semantic = References::semantic_type().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = semantic.shape() else { panic!() };
        let field = fields.iter().find(|field| field.name() == field_name).unwrap();
        let StructuredInfoTypeShape::Leaf(kind) = field.value_type().shape() else { panic!() };
        kind.as_str()
            .strip_prefix("data/generation-reference<")
            .and_then(|value| value.strip_suffix('>'))
            .unwrap()
            .to_string()
    }

    fn encoded_record(image: &[u8], resource: &[u8], text: &[u8]) -> Vec<u8> {
        let semantic = References::semantic_type().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = semantic.shape() else { panic!() };
        let leaf = |name: &str, encoded: &[u8]| {
            let field = fields.iter().find(|field| field.name() == name).unwrap();
            conduit_core::StructuredFieldValue::new(
                name,
                StructuredInfoValue::leaf(field.value_type().clone(), encoded.to_vec()).unwrap(),
            ).unwrap()
        };
        StructuredInfoValue::record(
            semantic.clone(),
            vec![leaf("image", image), leaf("resource", resource), leaf("text", text)],
        ).unwrap().canonical_bytes().unwrap()
    }

    #[test]
    fn valid_round_trip_and_leaf_ids_are_exact() {
        let image_kind = generation_content_kind("image");
        let text_kind = generation_content_kind("text");
        let generic = BoundedResourceRef {
            access_class: ResourceClassId::from("content/private@1"),
            extent: ResourceExtent { bytes: 80, items: Some(4) },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([9; 32]),
                expires_at: Some(TemporalInstant {
                    ticks: 100,
                    scale: TemporalScale::Milliseconds,
                    clock_basis: "clock/test@1".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                }),
            },
            ..raw_reference("media/archive@1")
        };
        let value = References::new(
            data_reference(&image_kind),
            generic,
            data_reference(&text_kind),
        ).unwrap();
        let encoded = value.clone().encode().unwrap();
        assert_eq!(References::decode(&encoded).unwrap(), value);

        let semantic = References::semantic_type().unwrap();
        let StructuredInfoTypeShape::Record { fields, .. } = semantic.shape() else { panic!() };
        let id = |name: &str| {
            let field = fields.iter().find(|field| field.name() == name).unwrap();
            let StructuredInfoTypeShape::Leaf(kind) = field.value_type().shape() else { panic!() };
            kind.clone()
        };
        assert_eq!(id("text"), data_reference_kind(&KindId::from(text_kind)));
        assert_eq!(id("image"), data_reference_kind(&KindId::from(image_kind)));
        assert_eq!(id("resource").as_str(), RESOURCE_REFERENCE_INFO_ID);
    }

    #[test]
    fn malformed_zero_digest_and_oversize_references_refuse() {
        let valid_image = raw_reference(&generation_content_kind("image")).encode().unwrap();
        let valid_text = raw_reference(&generation_content_kind("text")).encode().unwrap();
        let valid_resource = raw_reference("application/octet-stream@1").encode().unwrap();
        assert!(References::decode(&encoded_record(&[1], &valid_resource, &valid_text)).is_err());

        let mut zero_identity = valid_resource.clone();
        zero_identity[1..33].fill(0);
        assert!(References::decode(&encoded_record(&valid_image, &zero_identity, &valid_text)).is_err());
        let mut zero_version = valid_resource.clone();
        zero_version[33..65].fill(0);
        assert!(References::decode(&encoded_record(&valid_image, &zero_version, &valid_text)).is_err());
        assert!(References::decode(&encoded_record(&valid_image, &vec![0; 513], &valid_text)).is_err());
    }

    #[test]
    fn generation_content_access_lifetime_and_extent_must_match() {
        let generic = raw_reference("application/octet-stream@1").encode().unwrap();
        let valid_image = raw_reference(&generation_content_kind("image")).encode().unwrap();
        let text_kind = generation_content_kind("text");

        let wrong_content = raw_reference("value/bytes").encode().unwrap();
        assert!(References::decode(&encoded_record(&valid_image, &generic, &wrong_content)).is_err());
        let mut wrong_access = raw_reference(&text_kind);
        wrong_access.access_class = ResourceClassId::from("content/public@1");
        assert!(References::decode(&encoded_record(&valid_image, &generic, &wrong_access.encode().unwrap())).is_err());
        let mut expiring = raw_reference(&text_kind);
        expiring.lifetime.expires_at = Some(TemporalInstant {
            ticks: 100,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/test@1".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        });
        assert!(References::decode(&encoded_record(&valid_image, &generic, &expiring.encode().unwrap())).is_err());
        let mut item_extent = raw_reference(&text_kind);
        item_extent.extent.items = Some(1);
        assert!(References::decode(&encoded_record(&valid_image, &generic, &item_extent.encode().unwrap())).is_err());
    }
}
"#;
