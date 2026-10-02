use conduit_form::rust_binding::{
    generate_rust_bindings_with_codes_and_external_bindings, ExternalNativeRustBinding,
    RustBindingOptions,
};
use conduit_form::{
    check_syntax_document, generate_ecmascript_codes, parse_syntax_document, StartupCatalog,
};
use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=types.conduit");
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_value_kind_alias(
            "ResourceRef",
            conduit_form::rust_binding::semantic_core::kind_id(
                conduit_form::rust_binding::semantic_core::RESOURCE_REFERENCE_INFO_ID,
            ),
        )
        .expect("resource references are one exact portable leaf");
    let temporal_instant =
        conduit_time::NativeTemporalInstant::semantic_type().expect("temporal instant Type checks");
    catalog
        .insert_structured_type("TemporalInstant", temporal_instant.clone())
        .expect("temporal instant Type installs once");
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!("types.conduit")),
        &catalog,
    )
    .expect("data semantic Types must check");
    let temporal_identity = match temporal_instant.shape() {
        conduit_form::rust_binding::semantic_core::StructuredInfoTypeShape::Record {
            schema,
            ..
        } => schema.as_str().to_owned(),
        _ => panic!("temporal instant has one named record identity"),
    };
    let generated = generate_rust_bindings_with_codes_and_external_bindings(
        &checked.native_types,
        &checked.codes,
        &[temporal_instant],
        &[ExternalNativeRustBinding {
            semantic_identity: &temporal_identity,
            rust_type_path: "conduit_time::NativeTemporalInstant",
        }],
        &RustBindingOptions {
            boxed_variant_payloads: [
                "ObservationValue.sampled-signal".into(),
                "ObservationValue.tensor".into(),
                "SignalCadence.irregular".into(),
                "TabularQueryOutcomeFour.inline".into(),
            ]
            .into(),
            copy_record_types: [
                "DataGenerationNamespace".into(),
                "MeasurementPlotPoint".into(),
                "MeasurementPlotProfile".into(),
                "MeasurementRange".into(),
                "MeasurementHysteresisProfile".into(),
                "MeasurementThresholdPolicy".into(),
            ]
            .into(),
            copy_nominal_types: [
                "DataGenerationDigest".into(),
                "DatasetExampleIdentity".into(),
                "ScientificObservationIdentity".into(),
                "SignalIdentity".into(),
                "TensorResourceIdentity".into(),
            ]
            .into(),
            hash_nominal_types: [
                "DataGenerationDigest".into(),
                "DatasetExampleIdentity".into(),
                "ScientificObservationIdentity".into(),
                "SignalIdentity".into(),
                "TensorResourceIdentity".into(),
            ]
            .into(),
            copy_record_value_getters: ["DataGenerationNamespace".into()].into(),
            public_record_fields: [
                "AlignedTrainingView".into(),
                "CalibrationTransform".into(),
                "CoordinateFrame".into(),
                "ConcatenatedSignal".into(),
                "DatasetDescriptor".into(),
                "DatasetSplitMembership".into(),
                "MeasurementHysteresisProfile".into(),
                "MeasurementPlotProfile".into(),
                "MeasurementRange".into(),
                "MeasurementSample".into(),
                "MeasurementSummary".into(),
                "MeasurementThresholdDecision".into(),
                "MeasurementWindowProfile".into(),
                "MissingDataMask".into(),
                "ObservationSet".into(),
                "SignalWindow".into(),
                "SignalSummary".into(),
                "SampledSignal".into(),
                "ScientificObservation".into(),
                "TensorAxis".into(),
                "TensorSummary".into(),
                "TensorValue".into(),
            ]
            .into(),
            record_constructor_names: [("DataGenerationNamespace".into(), "from_digest".into())]
                .into(),
            record_constructor_orders: [(
                "MeasurementPlotSeries".into(),
                vec![
                    "projected-points".into(),
                    "source-sample-count".into(),
                    "omitted-sample-count".into(),
                ],
            )]
            .into(),
            ..RustBindingOptions::default()
        },
    )
    .expect("data semantic Types and codes must generate exact Rust bindings");
    let output_directory = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo supplies OUT_DIR"));
    let output = output_directory.join("semantic_types.rs");
    fs::write(output, generated.source).expect("write generated data bindings");
    let ecmascript = output_directory.join("codes.mjs");
    fs::write(&ecmascript, generate_ecmascript_codes(&checked.codes))
        .expect("write generated ECMAScript code bindings");
    println!(
        "cargo:rustc-env=CONDUIT_DATA_CODES_MJS={}",
        ecmascript.display()
    );
}
