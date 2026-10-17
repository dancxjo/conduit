use conduit_plot::{parse_syntax_document, Argument, BackStatement, ExpressionSyntax};
use serde_json::Value;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

fn path(source: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-glyph-inspection-{}-{nonce}.conduit",
        std::process::id()
    ));
    fs::write(&path, source).unwrap();
    path
}
fn command(path: &PathBuf, entrance: &str, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
    command.arg(entrance).arg(path);
    if json {
        command.arg("--json");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
fn decoded(value: &Value) -> conduit_core::StructuredInfoValue {
    let hex = value["canonical_value_hex"].as_str().unwrap();
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|offset| u8::from_str_radix(&hex[offset..offset + 2], 16).unwrap())
        .collect::<Vec<_>>();
    conduit_core::StructuredInfoValue::from_canonical_bytes(&bytes).unwrap()
}

#[test]
fn public_expansion_reports_sealed_glyph_values_and_native_refinement_nodes() {
    let source = "with text/pattern/notation as r\ntype Label = Text <= 32B ~ r/[A-Z]+/i\nplot example {\n ascii = r/[A-Z]+/i\n unicode = r⟦[A-Z]+⟧i\n}\n";
    let path = path(source);
    let report: Value = serde_json::from_slice(&command(&path, "expand", true).stdout).unwrap();
    assert_eq!(report["glyphs"].as_array().unwrap().len(), 3);
    let glyphs = report["glyphs"].as_array().unwrap();
    for glyph in glyphs {
        let start = glyph["source_span"]["start"].as_u64().unwrap() as usize;
        let end = glyph["source_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], glyph["authored"].as_str().unwrap());
        let start = glyph["payload_span"]["start"].as_u64().unwrap() as usize;
        let end = glyph["payload_span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&source[start..end], glyph["raw_payload"].as_str().unwrap());
        assert_eq!(glyph["import_path"], "text/pattern/notation");
        assert_eq!(glyph["ordinary_constructor"], "text/pattern-from-source");
        assert!(glyph["parser_contract"]
            .as_str()
            .unwrap()
            .contains("portable-pattern"));
        assert_eq!(glyph["family_identity"].as_str().unwrap().len(), 64);
        assert_eq!(glyph["notation_source_id"].as_str().unwrap().len(), 64);
        decoded(&glyph["checked_value"]);
    }
    assert_eq!(
        decoded(&glyphs[1]["checked_value"]),
        decoded(&glyphs[2]["checked_value"])
    );
    let human = String::from_utf8(command(&path, "expand", false).stdout).unwrap();
    assert!(human.contains("ordinary constructor: text/pattern-from-source"));
    assert!(human.contains("r⟦[A-Z]+⟧i"));
    let inspected = String::from_utf8(command(&path, "inspect", false).stdout).unwrap();
    assert!(inspected.contains("ordinary constructor: text/pattern-from-source"));
    fs::remove_file(path).unwrap();
}

#[test]
fn public_ipa_inspection_retains_exact_selected_basis_for_both_branches() {
    let quoted = include_str!("../../../semantics/speech/examples/ipa/quoted-phonemic.conduit");
    let document = parse_syntax_document(quoted);
    let BackStatement::NamedGear(gear) = &document.plots[0].back[0] else {
        panic!()
    };
    let fields = gear
        .invocation
        .arguments
        .iter()
        .map(|argument| {
            let Argument::Named { name, value, .. } = argument else {
                panic!()
            };
            if name.text == "request" {
                let ExpressionSyntax::Record { fields, .. } = &value.syntax else {
                    panic!()
                };
                let field = fields
                    .iter()
                    .find(|field| field.name.text == "provenance")
                    .unwrap();
                let span = field.value.span();
                (
                    "provenance".to_string(),
                    quoted[span.start..span.end].to_string(),
                )
            } else {
                (name.text.clone(), value.text.clone())
            }
        })
        .collect::<Vec<_>>();
    let context = fields
        .iter()
        .map(|(key, _)| format!("{key}: chosen-{key}"))
        .collect::<Vec<_>>()
        .join(", ");
    let declarations = fields
        .iter()
        .map(|(key, value)| format!(" chosen-{key} = {value}\n"))
        .collect::<String>();
    let source = format!("with speech/ipa/notation as ph using {{{context}}}\nplot example {{\n{declarations} phonetic = ph[ˈt͡ʃaː]\n phonemic = ph/ˈt͡ʃaː/\n}}\n");
    let path = path(&source);
    let report: Value = serde_json::from_slice(&command(&path, "expand", true).stdout).unwrap();
    let glyphs = report["glyphs"].as_array().unwrap();
    assert_eq!(glyphs.len(), 2);
    assert_eq!(
        glyphs[0]["ordinary_constructor"],
        "speech/phonetic-from-ipa"
    );
    assert_eq!(
        glyphs[1]["ordinary_constructor"],
        "speech/phonemic-from-ipa"
    );
    assert_ne!(
        decoded(&glyphs[0]["checked_value"]).value_type(),
        decoded(&glyphs[1]["checked_value"]).value_type()
    );
    assert_eq!(
        report["glyph_contexts"].as_array().unwrap().len(),
        fields.len()
    );
    for glyph in glyphs {
        assert_eq!(glyph["basis"].as_array().unwrap().len(), fields.len());
        for basis in glyph["basis"].as_array().unwrap() {
            let index = basis["context_index"].as_u64().unwrap() as usize;
            let value = &report["glyph_contexts"][index]["checked_value"];
            decoded(value);
            assert_eq!(basis["checked_value_digest"], value["value_digest"]);
            assert!(basis["local"].as_str().unwrap().starts_with("chosen-"));
        }
    }
    let human = String::from_utf8(command(&path, "inspect", false).stdout).unwrap();
    assert!(human.contains("basis provenance = chosen-provenance"));
    fs::remove_file(path).unwrap();
}
