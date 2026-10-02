use conduit_plot::rust_binding::{
    generate_rust_bindings_with_forms_and_external_bindings, ExternalNativeRustBinding,
    RustBindingOptions,
};
use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "Quantity",
            conduit_plot::rust_binding::semantic_core::kind_id(
                conduit_plot::rust_binding::semantic_core::QUANTITY_INFO_ID,
            ),
        )
        .expect("Quantity is one exact portable leaf");
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_plot::rust_binding::semantic_core::kind_id(
                conduit_plot::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let image_observation = conduit_human::ImageObservationReference::semantic_type()
        .expect("human image observation Type checks");
    let temporal_instant =
        conduit_time::NativeTemporalInstant::semantic_type().expect("temporal instant Type checks");
    catalog
        .insert_structured_type("ImageObservationReference", image_observation.clone())
        .expect("human image observation Type installs once");
    catalog
        .insert_structured_type("TemporalInstant", temporal_instant.clone())
        .expect("temporal instant Type installs once");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("presentation semantic Types must check");
    let external_identity =
        |value_type: &conduit_plot::rust_binding::semantic_core::StructuredInfoType| {
            match value_type.shape() {
                conduit_plot::rust_binding::semantic_core::StructuredInfoTypeShape::Nominal {
                    schema,
                    ..
                }
                | conduit_plot::rust_binding::semantic_core::StructuredInfoTypeShape::Record {
                    schema,
                    ..
                }
                | conduit_plot::rust_binding::semantic_core::StructuredInfoTypeShape::Variant {
                    schema,
                    ..
                } => schema.as_str().to_owned(),
                _ => panic!("external native Type has a named identity"),
            }
        };
    let image_observation_identity = external_identity(&image_observation);
    let temporal_instant_identity = external_identity(&temporal_instant);
    let generated = generate_rust_bindings_with_forms_and_external_bindings(
        &checked.native_types,
        &checked.type_forms,
        &[image_observation, temporal_instant],
        &[
            ExternalNativeRustBinding {
                semantic_identity: &image_observation_identity,
                rust_type_path: "conduit_human::ImageObservationReference",
            },
            ExternalNativeRustBinding {
                semantic_identity: &temporal_instant_identity,
                rust_type_path: "conduit_time::NativeTemporalInstant",
            },
        ],
        &RustBindingOptions {
            derive_serde_for_variants: true,
            serde_variant_exclusions: [
                "FaceUtteranceProvenance".into(),
                "PresentationCompositionKind".into(),
                "VisionColorSample".into(),
                "VisionDetectionSlot".into(),
                "VisionLandmarkSlot".into(),
                "VisionOptionalPixelRegion".into(),
                "VisualExperienceObservation".into(),
                "VisualImpressionDisposition".into(),
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
    .expect("presentation semantic Types and Forms must generate exact Rust bindings");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"))
        .join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated presentation bindings");
}
