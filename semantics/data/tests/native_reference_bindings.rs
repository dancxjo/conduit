use conduit_plot::rust_binding::{generate_rust_bindings, RustBindingOptions};
use std::fs;
use std::process::Command;

#[test]
fn generated_reference_bindings_enforce_exact_leaf_contracts() {
    let source = "type Image = Bytes\ntype References = {\n    text: &Text\n    image: &Image\n    resource: ResourceRef\n}\n";
    let mut catalog = conduit_plot::StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_core::kind_id(conduit_core::RESOURCE_REFERENCE_INFO_ID),
        )
        .unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &catalog)
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
            "[package]\nname = \"generated-reference-proof\"\nversion = \"0.0.0\"\nedition = \"2021\"\n\n[dependencies]\nconduit-plot = {{ path = {:?} }}\nconduit-data = {{ path = {:?} }}\n",
            repository.join("architecture/plot"),
            repository.join("semantics/data")
        ),
    )
    .unwrap();
    let execution = Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
        .args(["test", "--offline", "--manifest-path"])
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
    println!(
        "generated reference proof at {}:\n{}\n{}",
        directory.display(),
        String::from_utf8_lossy(&execution.stdout),
        String::from_utf8_lossy(&execution.stderr)
    );
    if std::env::var_os("CONDUIT_NATIVE_REFERENCE_PROOF_RETAIN").is_none() {
        fs::remove_dir_all(directory).unwrap();
    }
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
        // Start with checked, valid leaves, then replace only each length-delimited
        // leaf node. Malformed payloads must reach Native decoding without making
        // the checked constructor manufacture an invalid ResourceRef.
        let valid_image = raw_reference(&generation_content_kind("image")).encode().unwrap();
        let valid_text = raw_reference(&generation_content_kind("text")).encode().unwrap();
        let valid_resource = raw_reference("application/octet-stream@1").encode().unwrap();
        let mut encoded = StructuredInfoValue::record(
            semantic.clone(),
            vec![leaf("image", &valid_image), leaf("resource", &valid_resource), leaf("text", &valid_text)],
        ).unwrap().canonical_bytes().unwrap();
        assert!(References::decode(&encoded).is_ok());
        let checked = conduit_core::validate_canonical_structured_value(&encoded).unwrap();
        let mut replacements = Vec::new();
        for (name, payload) in [("image", image), ("resource", resource), ("text", text)] {
            let node = checked.record_field(name).unwrap().unwrap().value_node();
            assert_eq!(node[0], 0); // canonical leaf node: tag, u32 length, bytes
            assert_eq!(u32::from_le_bytes(node[1..5].try_into().unwrap()) as usize, node.len() - 5);
            let start = node.as_ptr() as usize - encoded.as_ptr() as usize;
            let mut replacement = vec![0];
            replacement.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_le_bytes());
            replacement.extend_from_slice(payload);
            replacements.push((start..start + node.len(), replacement));
        }
        // Descending offsets keep every other field and the exact Type intact.
        replacements.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, replacement) in replacements { encoded.splice(range, replacement); }
        encoded
    }

    fn resource_refusal(payload: &[u8], expected: conduit_core::ResourceReferenceRefusal,
        image: &[u8], text: &[u8]) {
        use conduit_core::{PrimitiveInfoRefusal, StructuredInfoRefusal};
        use conduit_plot::rust_binding::NativeBindingRefusal;
        let leaf_type = conduit_core::StructuredInfoType::leaf(RESOURCE_REFERENCE_INFO_ID.into()).unwrap();
        let structured = StructuredInfoRefusal::InvalidPrimitiveLeaf(
            PrimitiveInfoRefusal::ResourceReference(expected));
        // Checked construction must still refuse early; the independently forged
        // wire record below exercises the generated Native decoder separately.
        assert_eq!(StructuredInfoValue::leaf(leaf_type, payload.to_vec()).unwrap_err(), structured);
        assert_eq!(References::decode(&encoded_record(image, payload, text)).unwrap_err(),
            NativeBindingRefusal::InvalidValue(structured));
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
        resource_refusal(&zero_identity, conduit_core::ResourceReferenceRefusal::ZeroSemanticIdentity, &valid_image, &valid_text);
        let mut zero_version = valid_resource.clone();
        zero_version[33..65].fill(0);
        resource_refusal(&zero_version, conduit_core::ResourceReferenceRefusal::ZeroVersionIdentity, &valid_image, &valid_text);
        resource_refusal(&vec![0; 513], conduit_core::ResourceReferenceRefusal::EncodingTooLarge, &valid_image, &valid_text);
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
