use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn plot_path(label: &str, source: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock must follow the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "conduit-diagnostic-{label}-{}-{nonce}.conduit",
        std::process::id()
    ));
    fs::write(&path, source).expect("diagnostic fixture should be writable");
    path
}

fn diagnose(path: &Path, json: bool) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_conduit"));
    command.args(["check", path.to_str().expect("UTF-8 fixture path")]);
    if json {
        command.arg("--json");
    }
    command.output().expect("diagnostic command should launch")
}

#[test]
fn malformed_plot_human_and_json_render_the_same_owned_diagnostic() {
    let path = plot_path("malformed", "not a plot\n");
    let human = diagnose(&path, false);
    let machine = diagnose(&path, true);
    assert!(!human.status.success());
    assert!(!machine.status.success());

    let human_stdout = String::from_utf8(human.stdout).unwrap();
    let machine_stdout = String::from_utf8(machine.stdout).unwrap();
    let diagnostics: Value = serde_json::from_str(&machine_stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["schema_version"], 1);
    assert_eq!(diagnostic["code"], "CND-FRM-019");
    assert!(human_stdout.contains(diagnostic["code"].as_str().unwrap()));
    assert!(human_stdout.contains(diagnostic["summary"].as_str().unwrap()));
    assert!(!machine_stdout.contains(path.to_str().unwrap()));

    fs::remove_file(path).expect("diagnostic fixture should be removable");
}

#[test]
fn unsupported_kind_keeps_source_identity_and_exact_primary_span() {
    let path = plot_path("unsupported", "plot demo {\n  x: missing/kind\n}\n");
    let output = diagnose(&path, true);
    assert!(!output.status.success());
    let diagnostics: Value = serde_json::from_slice(&output.stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "CND-FRM-028");
    assert_eq!(diagnostic["source_document_id"].as_str().unwrap().len(), 64);
    assert_eq!(diagnostic["content_hash"].as_str().unwrap().len(), 64);
    assert!(diagnostic["primary_span"]["end"].as_u64().unwrap() > 0);

    fs::remove_file(path).expect("diagnostic fixture should be removable");
}

#[test]
fn quoted_ipa_refusal_preserves_the_authored_escape_in_human_and_json_diagnostics() {
    let source =
        include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit")
            .replace("ˈt͡ʃãː.n̩", r"t͡ʃ\n");
    let path = plot_path("ipa-escape", &source);
    let human = diagnose(&path, false);
    let machine = diagnose(&path, true);
    assert!(!human.status.success());
    assert!(!machine.status.success());
    let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "CND-SPC-IPA");
    let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
    let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], r"\n");
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(diagnostic["summary"].as_str().unwrap()));
    assert!(human.contains("CND-SPC-IPA"));
    fs::remove_file(path).unwrap();
}

#[test]
fn reusable_basis_refusal_locates_the_declared_revision_in_both_renderers() {
    let source = include_str!("../../../semantics/speech/examples/ipa/reusable-basis.conduit")
        .replacen(
            "revision: \"revision/test\"",
            "revision: \"revision/stale\"",
            1,
        );
    let path = plot_path("ipa-basis", &source);
    let human = diagnose(&path, false);
    let machine = diagnose(&path, true);
    assert!(!human.status.success());
    assert!(!machine.status.success());
    let diagnostics: Value = serde_json::from_slice(&machine.stdout).unwrap();
    let diagnostic = &diagnostics[0];
    assert_eq!(diagnostic["code"], "CND-SPC-IPA");
    let start = diagnostic["primary_span"]["start"].as_u64().unwrap() as usize;
    let end = diagnostic["primary_span"]["end"].as_u64().unwrap() as usize;
    assert_eq!(&source[start..end], "\"revision/stale\"");
    assert_eq!(diagnostic["source_document_id"].as_str().unwrap().len(), 64);
    let human = String::from_utf8(human.stdout).unwrap();
    assert!(human.contains(diagnostic["summary"].as_str().unwrap()));
    fs::remove_file(path).unwrap();
}

#[test]
fn shipped_glyphs_enter_public_check_and_missing_ipa_context_refuses() {
    let pattern = plot_path(
        "pattern-glyph",
        "with text/pattern/notation as r\nplot example {\n value = r/[A-Z]+/i\n}\n",
    );
    let output = diagnose(&pattern, true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let diagnostics: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(diagnostics, serde_json::json!([]));
    fs::remove_file(pattern).unwrap();

    let ipa = plot_path(
        "ipa-context-missing",
        "with speech/ipa/notation as ph\nplot example {\n value = ph/a/\n}\n",
    );
    let output = diagnose(&ipa, true);
    assert!(!output.status.success());
    let diagnostics: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(diagnostics[0]["code"], "CND-GLY-003");
    assert!(diagnostics[0]["primary_span"]["end"].as_u64().unwrap() > 0);
    fs::remove_file(ipa).unwrap();
}

#[test]
fn authored_ipa_context_passes_public_check_for_both_branches() {
    use conduit_plot::{Argument, BackStatement, ExpressionSyntax};
    for (quoted, spelling) in [
        (
            include_str!("../../../semantics/speech/examples/ipa/quoted-transcriptions.conduit"),
            "ph[ˈt͡ʃãː.n̩]",
        ),
        (
            include_str!("../../../semantics/speech/examples/ipa/quoted-phonemic.conduit"),
            "ph/ˈt͡ʃaː/",
        ),
    ] {
        let original = conduit_plot::parse_syntax_document(quoted);
        let BackStatement::NamedGear(gear) = &original.plots[0].back[0] else {
            panic!()
        };
        let mut selections = Vec::new();
        let mut declarations = String::new();
        for argument in &gear.invocation.arguments {
            let Argument::Named { name, value, .. } = argument else {
                panic!()
            };
            let (key, span) = if name.text == "request" {
                let ExpressionSyntax::Record { fields, .. } = &value.syntax else {
                    panic!()
                };
                (
                    "provenance",
                    fields
                        .iter()
                        .find(|field| field.name.text == "provenance")
                        .unwrap()
                        .value
                        .span(),
                )
            } else {
                (name.text.as_str(), value.span)
            };
            selections.push(format!("{key}: chosen-{key}"));
            declarations.push_str(&format!(
                "chosen-{key} = {}\n",
                &quoted[span.start..span.end]
            ));
        }
        let source = format!("with speech/ipa/notation as ph using {{{}}}\nplot example {{\n{declarations}value = {spelling}\n}}\n", selections.join(", "));
        let path = plot_path("ipa-authored-context", &source);
        let output = diagnose(&path, true);
        assert!(
            output.status.success(),
            "stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&output.stdout).unwrap(),
            serde_json::json!([])
        );
        fs::remove_file(path).unwrap();
    }
}
