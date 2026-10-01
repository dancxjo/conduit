use conduit_form::rust_binding::{generate_rust_bindings_with_codes, RustBindingOptions};
use conduit_form::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "Quantity",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::QUANTITY_INFO_ID,
            ),
        )
        .expect("Quantity is one exact portable leaf");
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("presentation semantic Types must check");
    let generated = generate_rust_bindings_with_codes(
        &checked.native_types,
        &checked.codes,
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: [
                "FaceUtteranceProvenance".into(),
                "PresentationCompositionKind".into(),
                "VisionColorSample".into(),
                "VisionDetectionSlot".into(),
                "VisionLandmarkSlot".into(),
            ]
            .into(),
            serde_record_types: ["GeneratedActionAffordance".into()].into(),
            serde_deny_unknown_record_types: ["GeneratedActionAffordance".into()].into(),
            copy_record_types: [
                "GraphicsPoint".into(),
                "LayoutFrame".into(),
                "LayoutRect".into(),
                "ThemeColor".into(),
            ]
            .into(),
            copy_record_value_getters: ["ThemeColor".into()].into(),
            public_record_fields: [
                "ApplicationAction".into(),
                "LayoutFrame".into(),
                "LayoutRect".into(),
                "PresentationCompositionRelation".into(),
            ]
            .into(),
            record_constructor_orders: [(
                "ThemeColor".into(),
                vec!["red".into(), "green".into(), "blue".into()],
            )]
            .into(),
            serde_variant_orders: [
                (
                    "GeneratedManifestationDisposition".into(),
                    [
                        "produced",
                        "truncated",
                        "refused",
                        "failed",
                        "cancelled",
                        "provider_lost",
                    ]
                    .map(String::from)
                    .into(),
                ),
                (
                    "GeneratedContentRole".into(),
                    ["speech", "presented_thought"].map(String::from).into(),
                ),
                (
                    "PresentationPlace".into(),
                    ["entrance", "program", "body"].map(String::from).into(),
                ),
                (
                    "PresentationAspect".into(),
                    ["structure", "plan", "play", "signs"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "PresentationDepth".into(),
                    ["primary", "context", "detail", "exact"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "NavigationRefusal".into(),
                    [
                        "stale_presentation",
                        "unknown_place",
                        "unknown_aspect",
                        "unknown_subject",
                        "unknown_relationship",
                        "history_exhausted",
                        "history_full",
                        "invalid_truth",
                    ]
                    .map(String::from)
                    .into(),
                ),
                (
                    "MaskWardrobeLifetime".into(),
                    ["wake", "body"].map(String::from).into(),
                ),
                (
                    "MaskPlanningDisposition".into(),
                    ["not_required", "replacement_required"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "MaskWardrobeError".into(),
                    [
                        "stale_revision",
                        "capacity_exceeded",
                        "duplicate_mask",
                        "unknown_mask",
                        "unknown_preference",
                        "invalid_lifetime_scope",
                        "invalid_route",
                        "duplicate_route",
                        "stale_selection",
                    ]
                    .map(String::from)
                    .into(),
                ),
                (
                    "FaceContributionRole".into(),
                    ["foreground", "tutorial", "inspection", "transient"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "GenerativeNarratorRole".into(),
                    ["transient_first_person_body_narrator"]
                        .map(String::from)
                        .into(),
                ),
                (
                    "PresentationCompositionKind".into(),
                    [
                        "group",
                        "contrast",
                        "juxtapose",
                        "emphasize",
                        "subordinate",
                        "associate",
                        "reveal_after",
                        "semantic",
                    ]
                    .map(String::from)
                    .into(),
                ),
            ]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("presentation semantic Types and codes must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated presentation bindings");
}
