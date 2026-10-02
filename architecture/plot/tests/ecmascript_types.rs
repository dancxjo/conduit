//! Cross-target proof: ECMAScript sees the same exact checked semantic Types.

use conduit_plot::{
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
             }\n\
             type FiniteFloat = F32 finite\n\
             type Probability = F32 finite in 0.0..=1.0\n\
             type Envelope<T> = {\n\
                 value: T\n\
                 history: sequence T <= 2\n\
             }\n\
             type TextEnvelope = Envelope<Text <= 16B>\n",
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
const finite = types.lookupType('FiniteFloat');
if (finite.shape.representation.identity !== 'value/ieee754-binary32') process.exit(28);
if (finite.contracts.length !== 1 || finite.contracts[0].maximumBytes !== 4) process.exit(29);
const probability = types.lookupType('Probability');
if (probability.contracts.length !== 1 || probability.contracts[0].identity.length === finite.contracts[0].identity.length && probability.contracts[0].identity.every((byte, index) => byte === finite.contracts[0].identity[index])) process.exit(30);
const negativeZero = types.inspectIeee('value/ieee754-binary32', Uint8Array.of(0, 0, 0, 128));
if (!negativeZero.signedZero || negativeZero.sign !== 1 || negativeZero.bits !== 0x80000000n) process.exit(31);
const quietNaN = types.inspectIeee('value/ieee754-binary32', Uint8Array.of(1, 0, 192, 127));
if (!quietNaN.nan || quietNaN.finite || quietNaN.bits !== 0x7fc00001n) process.exit(32);
const smallestSubnormal = types.inspectIeee('value/ieee754-binary32', Uint8Array.of(1, 0, 0, 0));
if (!smallestSubnormal.finite || smallestSubnormal.exponent !== 0n || smallestSubnormal.fraction !== 1n) process.exit(33);
const generic = types.lookupType('TextEnvelope');
if (generic.shape.kind !== 'record') process.exit(34);
const genericFields = Object.fromEntries(generic.shape.fields.map(field => [field.name, field.value]));
if (genericFields.value.identity !== 'value/text') process.exit(35);
if (genericFields.history.kind !== 'sequence' || genericFields.history.maximumItems !== 2) process.exit(36);
if (generic.contracts.map(contract => contract.path).sort().join(',') !== '.history[],.value') process.exit(37);
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
