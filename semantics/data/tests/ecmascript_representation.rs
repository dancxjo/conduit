//! Cross-target proof: JavaScript consumes the checked Conduitese mapping.

use std::process::Command;

#[test]
fn ecmascript_consumes_the_same_checked_terminal_representations() {
    let module = env!("CONDUIT_DATA_REPRESENTATIONS_MJS");
    let proof = r#"
const codec = await import(process.argv[1]);
const save = 'data/save-text-terminal';
const load = 'data/load-text-terminal';
if (codec.encode(save, 'wrong_content_kind')[0] !== 3) process.exit(10);
if (codec.decode(load, Uint8Array.of(6)) !== 'extent_mismatch') process.exit(11);
if (codec.representations[save].exactBytes !== 1) process.exit(12);
try { codec.decode(save, Uint8Array.of(4)); process.exit(13); }
catch (error) { if (error.kind !== 'invalid_tag') process.exit(14); }
"#;
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", proof, module])
        .output()
        .expect("Node.js is required by the browser target proof");
    assert!(
        output.status.success(),
        "ECMAScript representation proof failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
