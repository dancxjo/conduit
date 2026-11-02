#[path = "common/joining_host_package.rs"]
mod common;
use conduit_body_make::write_joining_host_package;
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Seek, SeekFrom},
};

#[test]
fn release_correspondence_modes_and_existing_js_reader() {
    let (release, spore, invitation, payloads) =
        common::fixture(&["conduit-linux-x86_64", "install.sh"]);
    let mut bytes = Vec::new();
    let receipt = write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |path| Ok(Cursor::new(payloads[path].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    assert_eq!(receipt.artifact_bytes, bytes.len() as u64);
    assert_eq!(receipt.image_content_sha256, release.image_content_digest());
    assert_ne!(
        receipt.artifact_content_sha256,
        receipt.image_content_sha256
    );
    assert_eq!(
        receipt.files.iter().map(|f| f.mode).collect::<Vec<_>>(),
        [0o100755, 0o100644, 0o100644]
    );
    // Actual existing JS consumer, stdin only: no fixture secret written to disk.
    let script = r#"import{readBodyBoundZip}from './targets/browser/workspace/creche-native-zip.mjs';let parts=[];for await(const chunk of process.stdin)parts.push(chunk);let r=readBodyBoundZip(Buffer.concat(parts));if(r.entries.size!==3||r.provision.invitation_provision.secret[0]!==7||r.modes.get('conduit-linux-x86_64')!==0o100755)throw Error('correspondence');"#;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    assert!(child.wait().unwrap().success());
}
#[test]
fn changed_missing_extra_and_expired_inputs_never_produce_receipts() {
    let (release, mut spore, mut invitation, payloads) = common::fixture(&["conduit"]);
    for changed in [
        b"other".to_vec(),
        Vec::new(),
        [payloads["conduit"].clone(), vec![0]].concat(),
    ] {
        assert!(write_joining_host_package(
            &release,
            &spore,
            &invitation,
            1,
            |_| Ok(Cursor::new(changed.clone())),
            &mut Vec::new(),
            || false
        )
        .is_err());
    }
    spore.body_id = "body/foreign".into();
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut Vec::new(),
        || false
    )
    .is_err());
    spore.body_id = "body/test".into();
    invitation.claim.expires_at_millis = 1;
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut Vec::new(),
        || false
    )
    .is_err());
}
struct ChangesOnRewind(Cursor<Vec<u8>>);
impl Read for ChangesOnRewind {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.0.read(b)
    }
}
impl Seek for ChangesOnRewind {
    fn seek(&mut self, p: SeekFrom) -> io::Result<u64> {
        self.0.get_mut()[0] ^= 1;
        self.0.seek(p)
    }
}
#[test]
fn source_changes_after_crc_and_mid_stream_cancellation_refuse() {
    let (release, spore, invitation, payloads) = common::fixture(&["conduit"]);
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(ChangesOnRewind(Cursor::new(payloads[p].clone()))),
        &mut Vec::new(),
        || false
    )
    .is_err());
    for limit in 0..15 {
        let calls = Cell::new(0);
        let mut output = Vec::new();
        let result = write_joining_host_package(
            &release,
            &spore,
            &invitation,
            1,
            |p| Ok(Cursor::new(payloads[p].clone())),
            &mut output,
            || {
                let n = calls.get();
                calls.set(n + 1);
                n >= limit
            },
        );
        if calls.get() > limit {
            assert!(result.is_err());
        }
    }
}
#[test]
fn entry_capacity_and_reserved_or_nested_names_refuse_before_output() {
    for paths in [vec!["conduit-spore.json"], vec!["nested/conduit"]] {
        let (release, spore, invitation, payloads) = common::fixture(&paths);
        let mut output = Vec::new();
        assert!(write_joining_host_package(
            &release,
            &spore,
            &invitation,
            1,
            |p| Ok(Cursor::new(payloads[p].clone())),
            &mut output,
            || false
        )
        .is_err());
        assert!(output.is_empty());
    }
    let paths = (0..17).map(|n| format!("payload{n}")).collect::<Vec<_>>();
    let (release, spore, invitation, payloads) =
        common::fixture(&paths.iter().map(String::as_str).collect::<Vec<_>>());
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut Vec::new(),
        || false
    )
    .is_err());
}

#[test]
fn typed_routed_invitation_survives_existing_js_consumer_without_legacy_conversion() {
    let (release, spore, mut invitation, payloads) = common::fixture(&["conduit"]);
    common::routed(&mut invitation);
    let mut bytes = Vec::new();
    write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    let script = r#"import assert from 'node:assert/strict';import{readBodyBoundZip}from './targets/browser/workspace/creche-native-zip.mjs';let parts=[];for await(const chunk of process.stdin)parts.push(chunk);let p=readBodyBoundZip(Buffer.concat(parts)).provision.invitation_provision;assert.equal(p.rendezvous.protocol,1);assert.equal(p.rendezvous.invitation_id,p.invitation_id);assert.deepEqual(p.rendezvous.candidates[0].authentication.transport_binding_sha256,Array(32).fill(22));assert.deepEqual(p.rendezvous_candidates,[]);"#;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    assert!(child.wait().unwrap().success());
    invitation.rendezvous.as_mut().unwrap().candidates[0].expires_at_millis = 10001;
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut Vec::new(),
        || false
    )
    .is_err());
}

#[test]
fn browser_bundle_uses_same_package_format_and_exact_output_correspondence() {
    let (release, mut spore, invitation, payloads) =
        common::fixture_for(&["runtime.wasm", "index.html"], true);
    let mut bytes = Vec::new();
    let receipt = write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut bytes,
        || false,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(&receipt).unwrap()["artifact_bytes"],
        bytes.len()
    );
    let script = r#"import assert from 'node:assert/strict';import{readBodyBoundZip}from './targets/browser/workspace/creche-native-zip.mjs';let parts=[];for await(const chunk of process.stdin)parts.push(chunk);let r=readBodyBoundZip(Buffer.concat(parts));assert.equal(r.provision.spore.output,'browser-bundle');assert.deepEqual(Array.from(r.entries.get('runtime.wasm')),[0,97,115,109,1,0,0,0]);"#;
    use std::{
        io::Write,
        process::{Command, Stdio},
    };
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut child = Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(root)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&bytes).unwrap();
    assert!(child.wait().unwrap().success());
    spore.make.output = conduit_host_make::SporeOutputKind::NativeBundle;
    assert!(write_joining_host_package(
        &release,
        &spore,
        &invitation,
        1,
        |p| Ok(Cursor::new(payloads[p].clone())),
        &mut Vec::new(),
        || false
    )
    .is_err());
}
