//! Cross-target proof: ECMAScript sees the same exact checked semantic Types.

use conduit_form::{
    check_syntax_document, generate_ecmascript_types, parse_syntax_document, StartupCatalog,
};
use std::{fs, process::Command};

#[test]
fn ecmascript_inspects_records_references_optionals_and_bounds() {
    let checked = check_syntax_document(
        &parse_syntax_document(
            "type UnitCode = U8\n\
             type Reading = {\n\
                 unit: UnitCode\n\
                 note: Text <= 16B?\n\
             }\n\
             type Digest = collection U8 = 32\n\
             type DigestEnvelope = {\n\
                 identity: Digest\n\
                 prior: Digest?\n\
                 ancestors: sequence Digest <= 16\n\
             }\n",
        ),
        &StartupCatalog::new(),
    )
    .expect("native semantic Types check");
    let module =
        std::env::temp_dir().join(format!("conduit-semantic-types-{}.mjs", std::process::id()));
    fs::write(&module, generate_ecmascript_types(&checked.native_types))
        .expect("generated module writes");
    let proof = r#"
const types = await import(process.argv[1]);
const reading = types.lookupType('Reading');
if (reading.shape.kind !== 'record') process.exit(10);
if (reading.shape.fields.map(field => field.name).join(',') !== 'note,unit') process.exit(11);
const note = reading.shape.fields[0];
if (note.value.kind !== 'optional') process.exit(12);
if (note.value.value.kind !== 'leaf' || note.value.value.identity !== 'value/text') process.exit(13);
const unit = reading.shape.fields[1];
if (unit.value.kind !== 'reference' || unit.value.name !== 'UnitCode') process.exit(14);
if (reading.contracts.length !== 1 || reading.contracts[0].path !== '.note?some') process.exit(15);
if (reading.contracts[0].maximumBytes !== 16) process.exit(16);
if (reading.canonicalType.length === 0 || reading.contracts[0].identity.length === 0) process.exit(17);
try { types.lookupType('Missing'); process.exit(18); }
catch (error) { if (!(error instanceof types.TypeRefusal)) process.exit(19); }
if (!Object.isFrozen(reading) || !Object.isFrozen(reading.shape.fields)) process.exit(20);
const digest = types.lookupType('Digest');
if (digest.shape.kind !== 'nominal' || digest.shape.representation.kind !== 'collection') process.exit(21);
if (digest.shape.representation.length !== 32) process.exit(22);
if (digest.shape.representation.element.identity !== 'value/u8') process.exit(23);
const envelope = types.lookupType('DigestEnvelope');
const fields = Object.fromEntries(envelope.shape.fields.map(field => [field.name, field.value]));
if (fields.identity.name !== 'Digest') process.exit(24);
if (fields.prior.kind !== 'optional' || fields.prior.value.name !== 'Digest') process.exit(25);
if (fields.ancestors.kind !== 'sequence' || fields.ancestors.maximumItems !== 16) process.exit(26);
if (fields.ancestors.element.name !== 'Digest') process.exit(27);
"#;
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "--eval",
            proof,
            module.to_str().expect("temporary module path is UTF-8"),
        ])
        .output()
        .expect("Node.js is required by the ECMAScript target proof");
    let _ = fs::remove_file(module);
    assert!(
        output.status.success(),
        "ECMAScript semantic Type proof failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
