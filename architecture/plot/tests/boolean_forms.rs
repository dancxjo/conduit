use conduit_plot::{
    check_syntax_document, parse_syntax_document, CheckedTypeFormStorage, StartupCatalog,
};

fn check(source: &str) -> conduit_plot::CheckedSyntaxDocument {
    let parsed = parse_syntax_document(source);
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    check_syntax_document(&parsed, &StartupCatalog::new()).unwrap()
}

#[test]
fn selected_bit_order_changes_form_identity_and_preserves_semantic_type() {
    let source =
        "type Flags = {\n    first: Boolean\n    second: Boolean\n}\nform flags = Flags as u8 bits\n";
    let a = check(source).type_forms.remove(0);
    let b = check(&format!("{source}    second\n    first\n"))
        .type_forms
        .remove(0);
    assert_eq!(a.value_type, b.value_type);
    assert_ne!(a.compatibility_id, b.compatibility_id);
    assert_eq!(a.storage, CheckedTypeFormStorage::BooleanBits(3));
    assert_eq!((a.exact_bytes, a.maximum_bytes), (1, 1));
}

#[test]
fn malformed_maps_and_laws_that_need_runtime_validation_are_refused() {
    for source in [
        "type Flags = {\n    x: U8\n}\nform flags = Flags as u8 bits\n",
        "type Flags = {\n    x: Boolean\n    y: Boolean\n}\nform flags = Flags as u8 bits\n    x\n    x\n",
        "type Flags = {\n    x: Boolean\n    y: Boolean\n}\nform flags = Flags as u8 bits\n    x\n",
        "type Flags = {\n    x: Boolean\n    where .x == true\n}\nform flags = Flags as u8 bits\n",
    ] {
        let parsed = parse_syntax_document(source);
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        assert!(check_syntax_document(&parsed, &StartupCatalog::new()).is_err());
    }
}

#[test]
fn generated_javascript_form_round_trips_and_rejects_reserved_bits() {
    use std::{fs, process::Command};
    let checked = check(include_str!(
        "../../../plots/seven-segment-mask/types.conduit"
    ));
    let javascript = conduit_plot::generate_ecmascript_forms(&checked.type_forms);
    let exercise = r#"
for (let byte=0; byte<128; byte++) {
  const value=decode('segment-pattern', Uint8Array.of(byte));
  if (encode('segment-pattern', value)[0] !== byte) throw Error('round trip');
}
for (let byte=128; byte<256; byte++) {
  let refused=false;
  try { decode('segment-pattern', Uint8Array.of(byte)); } catch(e) { refused=e.kind==='reserved_bits'; }
  if (!refused) throw Error('reserved bit accepted');
}
if (decode('seven-segment-selection', encode('seven-segment-selection','humidity'))!=='humidity') throw Error('enum regression');
let refused=false;
try { encode('segment-pattern',{a:1,b:false,c:false,d:false,e:false,f:false,g:false}); } catch(e) { refused=true; }
if (!refused) throw Error('nonboolean accepted');
"#;
    let path =
        std::env::temp_dir().join(format!("conduit-boolean-forms-{}.mjs", std::process::id()));
    fs::write(&path, format!("{javascript}\n{exercise}")).unwrap();
    let result = Command::new("node").arg(&path).output().unwrap();
    fs::remove_file(path).unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
