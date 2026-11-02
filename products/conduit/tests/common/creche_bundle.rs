//! Shared CPU fixture helpers; a software payload is not a deployable Host.
use conduit_body::PortableInvitation;
use conduit_body_make::SporeManifest;
use conduit_host_make::{
    verify_host_release_bytes, HostImage, HostReleaseBounds, ReleaseTargetProfile,
    VerifiedHostReleaseBytes,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
};

pub struct PrivateDirectory(pub PathBuf);
impl PrivateDirectory {
    pub fn new(path: PathBuf) -> Self {
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path)
    }
}
impl Drop for PrivateDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
pub fn native_release(image: &HostImage, payload: &[u8]) -> VerifiedHostReleaseBytes {
    let digest = format!("sha256:{:x}", Sha256::digest(payload));
    let aggregate = format!(
        "sha256:{:x}",
        Sha256::digest(format!(
            "conduit.release/host-bundle-content@1\0fixture.data\0{digest}\n"
        ))
    );
    let m = &image.manifest;
    let manifest=serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.release/host-bundle@1","target_id":m.target,"make_package_id":m.make_package_id,"output":"native-bundle",
        "builder_adapter":m.builder_adapter,"deployment_adapter":m.deployment_adapter,"source_identity":m.source_identity,
        "bundle_sha256":aggregate,"files":[{"path":"fixture.data","bytes":payload.len(),"sha256":digest,"media_type":"application/octet-stream"}]
    })).unwrap();
    verify_host_release_bytes(
        &manifest,
        &ReleaseTargetProfile {
            target_id: &m.target,
            package_id: &m.make_package_id,
            output: "native-bundle",
            builder_adapter: &m.builder_adapter,
            deployment_adapter: m.deployment_adapter.as_deref().unwrap(),
        },
        &m.source_identity,
        HostReleaseBounds {
            maximum_manifest_bytes: 65536,
            maximum_files: 1,
            maximum_file_bytes: 1024,
            maximum_bundle_bytes: 1024,
        },
        |name| {
            assert_eq!(name, "fixture.data");
            Ok(Cursor::new(payload))
        },
        || false,
    )
    .unwrap()
}

pub fn assert_seed_package(
    artifact: &mut File,
    spore: &SporeManifest,
    invitation: &PortableInvitation,
    receipt: &[u8],
    payload: &[u8],
) {
    artifact.rewind().unwrap();
    let mut bytes = Vec::new();
    artifact.read_to_end(&mut bytes).unwrap();
    artifact.rewind().unwrap();
    assert_eq!(&bytes[..4], b"PK\x03\x04");
    let receipt: serde_json::Value = serde_json::from_slice(receipt).unwrap();
    assert_eq!(receipt["artifact_bytes"], bytes.len() as u64);
    assert_eq!(
        receipt["artifact_content_sha256"],
        format!("sha256:{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(receipt["image_content_sha256"], spore.image_content_digest);
    assert_ne!(
        receipt["artifact_content_sha256"],
        receipt["image_content_sha256"]
    );
    let script = r#"
import {readBodyBoundZip} from './targets/browser/workspace/creche-native-zip.mjs';
import {createHash} from 'node:crypto';
let input='';for await(const part of process.stdin) input+=part;
const expected=JSON.parse(input);const decoded=readBodyBoundZip(new Uint8Array(expected.bytes));
const p=decoded.provision;const payload=decoded.entries.get('fixture.data');
console.log(JSON.stringify({body_id:p.spore.body_id,spore_id:p.spore.spore_id,invitation_id:p.invitation_provision.invitation_id,
  secret_matches:JSON.stringify(p.invitation_provision.secret)===JSON.stringify(expected.secret),
  payload_sha256:payload?'sha256:'+createHash('sha256').update(payload).digest('hex'):null,
  mode:decoded.modes.get('fixture.data'),files:decoded.entries.size}));
"#;
    let mut child = std::process::Command::new("node")
        .args(["--input-type=module", "-e", script])
        .current_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let mut input =
        serde_json::to_vec(&serde_json::json!({"bytes":bytes,"secret":invitation.secret})).unwrap();
    child.stdin.take().unwrap().write_all(&input).unwrap();
    input.fill(0);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "existing ZIP reader refused package: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(summary["body_id"], spore.body_id);
    assert_eq!(summary["spore_id"], spore.spore_id);
    assert_eq!(
        summary["invitation_id"],
        invitation.claim.invitation_id.as_str()
    );
    assert_eq!(summary["secret_matches"], true);
    assert_eq!(
        summary["payload_sha256"],
        format!("sha256:{:x}", Sha256::digest(payload))
    );
    assert_eq!(summary["mode"], 0o100644);
    assert_eq!(summary["files"], 2);
}
