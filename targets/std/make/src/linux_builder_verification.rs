//! Source snapshot and final deployable-byte binding checks.
use super::process::Execution;
use conduit_host_make::{BuilderBounds, BuilderRefusal, HostImage};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Command,
};

pub(super) fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn verify_source(
    root: &Path,
    expected: &str,
    bounds: &BuilderBounds,
    execution: &mut Execution<'_>,
) -> Result<String, BuilderRefusal> {
    let mut head = Command::new("git");
    head.arg("-C").arg(root).args(["rev-parse", "HEAD"]);
    let revision = String::from_utf8(execution.run(&mut head)?.stdout)
        .map_err(|_| BuilderRefusal::StaleRevision)?
        .trim()
        .to_owned();
    if revision.len() != 40 || !revision.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(BuilderRefusal::StaleRevision);
    }
    let mut status = Command::new("git");
    status
        .arg("-C")
        .arg(root)
        .args(["status", "--porcelain=v1", "--untracked-files=all"]);
    if !execution.run(&mut status)?.stdout.is_empty() {
        return Err(BuilderRefusal::DirtySource);
    }
    let mut names = Command::new("git");
    names.arg("-C").arg(root).args([
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    ]);
    let names = execution.run(&mut names)?.stdout;
    let mut hash = Sha256::new();
    let mut files = 0_u32;
    let mut bytes = 0_u64;
    for name in names.split(|c| *c == 0).filter(|n| !n.is_empty()) {
        let relative = std::str::from_utf8(name).map_err(|_| BuilderRefusal::MalformedArtifact)?;
        let path = safe_file(root, relative)?;
        files = files
            .checked_add(1)
            .ok_or(BuilderRefusal::SourceBoundExhausted)?;
        if files > bounds.maximum_source_files {
            return Err(BuilderRefusal::SourceBoundExhausted);
        }
        let remaining = bounds.maximum_source_bytes.saturating_sub(bytes);
        let content = read_bounded(
            &path,
            remaining,
            BuilderRefusal::SourceBoundExhausted,
            &mut || execution.check_progress(),
        )?;
        bytes += content.len() as u64;
        if files.is_multiple_of(256) {
            execution.check_bounds()?;
        }
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        hash.update((content.len() as u64).to_le_bytes());
        for chunk in content.chunks(65536) {
            execution.check_progress()?;
            hash.update(chunk);
        }
    }
    if format!("git:{revision}+sha256:{:x}", hash.finalize()) != expected {
        return Err(BuilderRefusal::StaleRevision);
    }
    Ok(revision)
}

pub(super) fn verify_output(
    root: &Path,
    expected: &HostImage,
    image_file: &str,
    bounds: &BuilderBounds,
    progress: &mut dyn FnMut() -> Result<(), BuilderRefusal>,
) -> Result<(PathBuf, u64, String), BuilderRefusal> {
    let manifest = read_bounded(
        &safe_file(root, "build-manifest.json")?,
        bounds.maximum_evidence_bytes,
        BuilderRefusal::BoundExhausted,
        progress,
    )?;
    let manifest: Value =
        serde_json::from_slice(&manifest).map_err(|_| BuilderRefusal::MalformedArtifact)?;
    let description = read_bounded(
        &safe_file(root, "resolved-image.json")?,
        bounds.maximum_evidence_bytes,
        BuilderRefusal::BoundExhausted,
        progress,
    )?;
    let resolved: HostImage =
        serde_json::from_slice(&description).map_err(|_| BuilderRefusal::MalformedArtifact)?;
    conduit_host_make::verify_image_binding(&resolved, &description)
        .map_err(|_| BuilderRefusal::MalformedArtifact)?;
    if &resolved != expected {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    let check =
        |field: &str, value: &str| manifest.get(field).and_then(Value::as_str) == Some(value);
    let m = &expected.manifest;
    let resolved_build: conduit_host_make::BuildManifest =
        serde_json::from_value(manifest["resolved_build"].clone())
            .map_err(|_| BuilderRefusal::MalformedArtifact)?;
    if !check("schema", "conduit.host/target-build-manifest@3")
        || !check("artifact_role", "product-host")
        || !check("profile_id", &m.profile_id)
        || !check("build_id", &m.build_id)
        || !check("target", &m.target)
        || !check("source_identity", &m.source_identity)
        || !check("toolchain_identity", &m.toolchain_identity)
        || !check("resolved_description_binding", &m.image_id)
        || manifest["boot_assets"]["limine_version"].as_str() != Some(super::assets::LIMINE_VERSION)
        || manifest["boot_assets"]["limine_archive_sha256"].as_str()
            != Some(super::assets::LIMINE_SHA256)
        || resolved_build != *m
    {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    verify_receipt(
        root,
        &manifest["kernel"],
        "conduitos-kernel.elf",
        "freestanding-kernel",
        bounds.maximum_output_bytes,
        progress,
    )?;
    let image = verify_receipt(
        root,
        &manifest["image"],
        image_file,
        "final-bootable-image",
        bounds.maximum_output_bytes,
        progress,
    )?;
    if !check("image_id", &format!("image:{}", image.2)) {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    // Host make descriptions and raw kernel ELFs cannot masquerade as bootable
    // ISO media: the reviewed operation emits ISO9660 with a primary descriptor.
    let mut file = fs::File::open(&image.0).map_err(|_| BuilderRefusal::MalformedArtifact)?;
    use std::io::{Seek, SeekFrom};
    progress()?;
    file.seek(SeekFrom::Start(32769))
        .map_err(|_| BuilderRefusal::MalformedArtifact)?;
    let mut signature = [0_u8; 5];
    file.read_exact(&mut signature)
        .map_err(|_| BuilderRefusal::MalformedArtifact)?;
    progress()?;
    if &signature != b"CD001" {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    Ok(image)
}
fn verify_receipt(
    root: &Path,
    receipt: &Value,
    name: &str,
    role: &str,
    limit: u64,
    progress: &mut dyn FnMut() -> Result<(), BuilderRefusal>,
) -> Result<(PathBuf, u64, String), BuilderRefusal> {
    if receipt["file"].as_str() != Some(name) || receipt["role"].as_str() != Some(role) {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    let path = safe_file(root, name)?;
    let mut file = fs::File::open(&path).map_err(|_| BuilderRefusal::MalformedArtifact)?;
    let mut hash = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 65536];
    loop {
        progress()?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| BuilderRefusal::MalformedArtifact)?;
        if n == 0 {
            break;
        }
        bytes = bytes
            .checked_add(n as u64)
            .ok_or(BuilderRefusal::BoundExhausted)?;
        if bytes > limit {
            return Err(BuilderRefusal::BoundExhausted);
        }
        hash.update(&buffer[..n]);
    }
    let sha256 = format!("sha256:{:x}", hash.finalize());
    if bytes == 0 || receipt["sha256"].as_str() != sha256.strip_prefix("sha256:") {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    Ok((path, bytes, sha256))
}
pub(super) fn read_bounded(
    path: &Path,
    limit: u64,
    exhausted: BuilderRefusal,
    progress: &mut dyn FnMut() -> Result<(), BuilderRefusal>,
) -> Result<Vec<u8>, BuilderRefusal> {
    let mut file = fs::File::open(path).map_err(|_| BuilderRefusal::MalformedArtifact)?;
    let mut result = Vec::new();
    let mut buffer = [0_u8; 65536];
    loop {
        progress()?;
        let n = file
            .read(&mut buffer)
            .map_err(|_| BuilderRefusal::MalformedArtifact)?;
        if n == 0 {
            break;
        }
        if (result.len() as u64).saturating_add(n as u64) > limit {
            return Err(exhausted);
        }
        result.extend_from_slice(&buffer[..n]);
    }
    Ok(result)
}

fn safe_file(root: &Path, relative: &str) -> Result<PathBuf, BuilderRefusal> {
    let relative = Path::new(relative);
    if relative
        .components()
        .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    let mut path = root.to_owned();
    for component in relative.components() {
        path.push(component);
        if path
            .symlink_metadata()
            .map_err(|_| BuilderRefusal::MalformedArtifact)?
            .file_type()
            .is_symlink()
        {
            return Err(BuilderRefusal::MalformedArtifact);
        }
    }
    if !path.is_file() {
        return Err(BuilderRefusal::MalformedArtifact);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn receipt_rejects_description_bytes_and_wrong_digest() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-receipt-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("image.json"), b"{}").unwrap();
        let receipt =
            serde_json::json!({"file":"image.json","role":"description","sha256":"a".repeat(64)});
        assert!(matches!(
            verify_receipt(
                &root,
                &receipt,
                "conduitos-x86_64.iso",
                "final-bootable-image",
                65536,
                &mut || Ok(())
            ),
            Err(BuilderRefusal::MalformedArtifact)
        ));
        let receipt =
            serde_json::json!({"file":"image.json","role":"description","sha256":"a".repeat(64)});
        assert!(matches!(
            verify_receipt(
                &root,
                &receipt,
                "image.json",
                "description",
                65536,
                &mut || Ok(())
            ),
            Err(BuilderRefusal::MalformedArtifact)
        ));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn exact_artifact_digest_detects_mutated_media() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-digest-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let content = b"exact artifact bytes";
        fs::write(root.join("media.iso"), content).unwrap();
        let receipt = serde_json::json!({"file":"media.iso", "role":"final-bootable-image", "sha256":digest(content).trim_start_matches("sha256:")});
        let verified = verify_receipt(
            &root,
            &receipt,
            "media.iso",
            "final-bootable-image",
            65536,
            &mut || Ok(()),
        )
        .unwrap();
        assert_eq!(verified.1, content.len() as u64);
        fs::write(root.join("media.iso"), b"mutated").unwrap();
        assert!(matches!(
            verify_receipt(
                &root,
                &receipt,
                "media.iso",
                "final-bootable-image",
                65536,
                &mut || Ok(())
            ),
            Err(BuilderRefusal::MalformedArtifact)
        ));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn verification_checks_cancellation_and_deadline_between_chunks() {
        let root =
            std::env::temp_dir().join(format!("conduit-linux-hash-cancel-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let content = vec![42; 3 * 65536];
        let path = root.join("media.iso");
        fs::write(&path, &content).unwrap();
        let receipt = serde_json::json!({"file":"media.iso", "role":"final-bootable-image", "sha256":digest(&content).trim_start_matches("sha256:")});
        for refusal in [BuilderRefusal::Interrupted, BuilderRefusal::BoundExhausted] {
            let mut chunks = 0;
            let mut progress = || {
                chunks += 1;
                if chunks == 2 {
                    Err(refusal.clone())
                } else {
                    Ok(())
                }
            };
            assert_eq!(
                verify_receipt(
                    &root,
                    &receipt,
                    "media.iso",
                    "final-bootable-image",
                    4 * 65536,
                    &mut progress
                ),
                Err(refusal.clone())
            );
            assert_eq!(chunks, 2);
            let mut chunks = 0;
            let mut progress = || {
                chunks += 1;
                if chunks == 2 {
                    Err(refusal.clone())
                } else {
                    Ok(())
                }
            };
            // Source-size refusal must not replace deadline/cancellation truth.
            assert_eq!(
                read_bounded(
                    &path,
                    4 * 65536,
                    BuilderRefusal::SourceBoundExhausted,
                    &mut progress
                ),
                Err(refusal)
            );
            assert_eq!(chunks, 2);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn artifact_path_cannot_escape_or_follow_a_symlink() {
        let root = std::env::temp_dir().join(format!("conduit-linux-path-{}", std::process::id()));
        fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", root.join("artifact")).unwrap();
        assert!(safe_file(&root, "artifact").is_err());
        assert!(safe_file(&root, "../artifact").is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
