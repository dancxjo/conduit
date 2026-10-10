use super::*;

fn expand_source(label: &str, source: &str) -> (String, serde_json::Value) {
    let path = std::env::temp_dir().join(format!(
        "conduit-type-expansion-{label}-{}.conduit",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let human = run(&path, false).unwrap();
    let json = run(&path, true).unwrap();
    let inspected = crate::inspection::inspect(&path).unwrap();
    assert!(inspected.contains("checked Type:"));
    std::fs::remove_file(path).unwrap();
    (human, serde_json::from_str(&json).unwrap())
}

#[test]
fn original_arguments_and_evaluated_nested_extents_share_one_checked_view() {
    let source = "# π and IPA t͡ʃ retain UTF-8 source offsets\ntype Vector<T, N: U16> = collection T = N\ntype Window<H: U16, D: U16> = {\n window: Vector<U8, (H + 1) * D>\n events: sequence U16 in 0..=D\n}\ntype Value = Window<2, 64>\n";
    let (human, json) = expand_source("nested", source);
    let value = &json["native_types"][0];
    assert_eq!(value["name"], "Value");
    assert_eq!(value["authored"], "type Value = Window<2, 64>");
    let start = value["source_span"]["start"].as_u64().unwrap() as usize;
    let end = value["source_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], value["authored"].as_str().unwrap());
    let fields = value["representation"]["fields"].as_array().unwrap();
    let window = fields
        .iter()
        .find(|field| field["name"] == "window")
        .unwrap();
    assert_eq!(
        window["representation"]["representation"]["exact_items"],
        192
    );
    let events = fields
        .iter()
        .find(|field| field["name"] == "events")
        .unwrap();
    assert_eq!(events["representation"]["constraint"], "sequence");
    assert_eq!(events["representation"]["minimum_items"], 0);
    assert_eq!(events["representation"]["maximum_items"], 64);
    assert_eq!(json["type_families"][0]["parameters"][0]["kind"], "Type");
    assert_eq!(json["type_families"][0]["parameters"][1]["kind"], "Info");
    assert!(human.contains("Window<2, 64>"));
    assert!(human.contains("\"exact_items\":192"));
    assert!(human.contains("Type family `Vector`"));
}

#[test]
fn equivalent_argument_spelling_preserves_checked_identity_and_distinguishes_source() {
    let source = "type Vector<N: U16> = collection U8 = N\ntype Value = Vector<32 + 32>\n";
    let (_, expression) = expand_source("expression", source);
    let (_, literal) = expand_source("literal", &source.replace("32 + 32", "64"));
    assert_eq!(
        expression["native_types"][0]["checked_identity"],
        literal["native_types"][0]["checked_identity"]
    );
    assert_eq!(
        expression["native_types"][0]["representation"],
        literal["native_types"][0]["representation"]
    );
    assert_ne!(
        expression["source_document_id"],
        literal["source_document_id"]
    );
    assert_ne!(
        expression["native_types"][0]["authored"],
        literal["native_types"][0]["authored"]
    );
}

#[test]
fn imported_family_view_retains_consumer_import_spans_and_closed_owner_identity() {
    use conduit_plot::{
        check_syntax_document, parse_syntax_document, CheckedPackageBundle, PackageExportCatalog,
        PackageMemberSource, StartupCatalog,
    };
    let manifest_source = "pack example/types (\n version = 1.0.0\n) {\n ship Vector\n}\n";
    let manifest = parse_syntax_document(manifest_source);
    let sources = [PackageMemberSource {
        path: "main",
        source: "type Vector<N: U16> = collection U8 = N\n",
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest_source, &manifest.packages[0], &sources)
            .unwrap();
    let exports = PackageExportCatalog::from_bundle(
        &bundle,
        manifest_source,
        &manifest.packages[0],
        &sources,
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    let source = "# λ\nwith example/types/Vector as Samples\ntype Value = Samples<32 + 32>\n";
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &catalog).unwrap();
    let types = serde_json::to_value(native_types::types(&syntax, &checked).unwrap()).unwrap();
    let imports = serde_json::to_value(native_types::imports(&syntax, &catalog)).unwrap();
    assert_eq!(types[0]["authored"], "type Value = Samples<32 + 32>");
    assert_eq!(
        types[0]["representation"]["representation"]["exact_items"],
        64
    );
    assert_eq!(
        types[0]["checked_identity"],
        checked
            .native_types
            .iter()
            .find(|value| value.name == "Value")
            .unwrap()
            .identity
            .as_str()
    );
    assert_eq!(imports[0]["path"], "example/types/Vector");
    assert_eq!(imports[0]["alias"], "Samples");
    assert_eq!(imports[0]["owner_sources"][0]["module_path"], "main");
    assert_eq!(imports[0]["owner_sources"][0]["declaration_name"], "Vector");
    assert_eq!(
        imports[0]["owner_sources"][0]["source_document_id"],
        parse_syntax_document(sources[0].source)
            .source_document_id()
            .as_str()
    );
    assert_eq!(
        imports[0]["owner_sources"][0]["package_content_digest"]
            .as_str()
            .unwrap()
            .len(),
        64
    );
    let start = imports[0]["source_span"]["start"].as_u64().unwrap() as usize;
    let end = imports[0]["source_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], "with example/types/Vector as Samples");
}

#[test]
fn repeated_admitted_types_refuse_excessive_inspection_expansion() {
    use conduit_plot::{check_syntax_document, parse_syntax_document, StartupCatalog};
    let mut source = String::from("type InspectionBlock = {\n");
    for field in 0..64 {
        source.push_str(&format!(" field{field}: U8\n"));
    }
    source.push_str("}\n");
    for alias in 0..260 {
        source.push_str(&format!("type InspectionCopy{alias} = InspectionBlock\n"));
    }
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let refusal = native_types::types(&syntax, &checked).unwrap_err();
    assert!(refusal.contains("inspection exceeds"), "{refusal}");
}

#[test]
fn closed_type_import_reports_original_owner_source() {
    use conduit_plot::{
        parse_syntax_document, CheckedPackageBundle, PackageExportCatalog, PackageMemberSource,
        StartupCatalog,
    };
    let manifest_source = "pack example/domain (\n version = 1.0.0\n) {\n ship Dimension\n}\n";
    let manifest = parse_syntax_document(manifest_source);
    let sources = [PackageMemberSource {
        path: "original/domain",
        source: "# Ω\ntype Dimension = U16 in 1..=64\n",
    }];
    let bundle =
        CheckedPackageBundle::from_sources(manifest_source, &manifest.packages[0], &sources)
            .unwrap();
    let exports = PackageExportCatalog::from_bundle(
        &bundle,
        manifest_source,
        &manifest.packages[0],
        &sources,
    )
    .unwrap();
    let mut catalog = StartupCatalog::new();
    exports.install_shipped_types(&mut catalog).unwrap();
    let syntax =
        parse_syntax_document("with example/domain/Dimension as Width\ntype Value = Width\n");
    let imports = native_types::imports(&syntax, &catalog);
    let json = serde_json::to_value(&imports).unwrap();
    assert_eq!(
        json[0]["owner_sources"][0]["module_path"],
        "original/domain"
    );
    assert_eq!(json[0]["owner_sources"][0]["declaration_name"], "Dimension");
    let report = ExpansionReport {
        schema: "conduit.source-sugar-expansion@1",
        boundary: "checked Source",
        source_document_id: "consumer",
        expansions: Vec::new(),
        native_types: Vec::new(),
        type_families: Vec::new(),
        imports,
        glyphs: Vec::new(),
        glyph_contexts: Vec::new(),
    };
    let human = render_human(&report);
    assert!(human.contains("owner `Dimension` in original/domain:2:1"));
    assert!(human.contains(
        parse_syntax_document(sources[0].source)
            .source_document_id()
            .as_str()
    ));
}
