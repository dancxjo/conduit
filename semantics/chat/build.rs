use conduit_plot::rust_binding::{generate_rust_bindings_with_forms, RustBindingOptions};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{collections::BTreeSet, env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_plot::rust_binding::semantic_core::kind_id(
                conduit_plot::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("chat semantic Types must check");
    let generated = generate_rust_bindings_with_forms(
        &checked.native_types,
        &checked.type_forms,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            record_constructor_orders: [(
                "ConversationRequestEvidence".into(),
                vec![
                    "request-identity".into(),
                    "body-id".into(),
                    "wake-id".into(),
                    "wake-sequence".into(),
                    "context-revision".into(),
                    "model-context-sha256".into(),
                    "private-prompt-retained".into(),
                ],
            )]
            .into(),
            serde_record_types: ["ConversationRequestEvidence".into()].into(),
            public_record_fields: [
                "LiveConversationFlowProjection".into(),
                "LiveConversationStage".into(),
                "RecognitionEvidenceView".into(),
                "SpeechCommitEvidenceView".into(),
            ]
            .into(),
            serde_variant_exclusions: BTreeSet::from(["MessageAttachmentSlot".into()]),
            ..RustBindingOptions::default()
        },
    )
    .expect("chat semantic Types and Forms must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated chat bindings");
}
