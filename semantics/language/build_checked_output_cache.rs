//! Opt-in reuse of a parent-reviewed, content-addressed complete checked output.
//! Not wired into build.rs until the manifest and converter parity are reviewed.
//! The compiled manifest digest is a trust anchor, not an environment assertion.
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
};
const MANIFEST: &str = "checked-output-cache.manifest";
const RECEIPT: &str = "checked-output-cache-receipt.txt";
const MAXIMUM_MANIFEST_BYTES: usize = 2 * 1024 * 1024;
const MAXIMUM_FILES: usize = 8192;
const MAXIMUM_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAXIMUM_OUTPUT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
#[derive(Debug)]
pub(super) struct CacheRefusal(pub(super) &'static str);
type Result<T> = std::result::Result<T, CacheRefusal>;
#[derive(Debug)]
pub(super) struct CacheReceipt {
    pub(super) output_files: usize,
    pub(super) output_bytes: u64,
    pub(super) manifest_sha256: String,
}
struct Entry<'a> {
    digest: &'a str,
    bytes: u64,
    path: &'a str,
}
fn hexadecimal(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    result
}
fn digest(bytes: &[u8]) -> String {
    hexadecimal(&Sha256::digest(bytes))
}
fn safe_path(path: &str, basename: bool) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.bytes().any(|b| b.is_ascii_whitespace())
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && (!basename || Path::new(path).components().count() == 1)
}
fn entry(line: &str) -> Result<(&str, Entry<'_>)> {
    let mut pieces = line.split(' ');
    let kind = pieces.next().ok_or(CacheRefusal("manifest kind"))?;
    let digest = pieces.next().ok_or(CacheRefusal("manifest digest"))?;
    let bytes = pieces
        .next()
        .ok_or(CacheRefusal("manifest extent"))?
        .parse::<u64>()
        .map_err(|_| CacheRefusal("manifest extent"))?;
    let path = pieces.next().ok_or(CacheRefusal("manifest path"))?;
    if pieces.next().is_some()
        || digest.len() != 64
        || !digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || bytes > MAXIMUM_FILE_BYTES
        || !safe_path(path, kind != "input")
        || (kind == "output" && matches!(path, MANIFEST | RECEIPT))
        || !matches!(kind, "input" | "output" | "provenance")
    {
        return Err(CacheRefusal("manifest entry"));
    }
    Ok((
        kind,
        Entry {
            digest,
            bytes,
            path,
        },
    ))
}
fn open_regular(path: &Path, bytes: u64) -> Result<File> {
    let metadata = fs::symlink_metadata(path).map_err(|_| CacheRefusal("missing file"))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() != bytes {
        return Err(CacheRefusal("file extent or kind"));
    }
    File::open(path).map_err(|_| CacheRefusal("open file"))
}
fn transfer(
    mut source: File,
    mut destination: Option<&mut File>,
    expected: &Entry<'_>,
) -> Result<()> {
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    let mut bytes = 0u64;
    loop {
        let count = source
            .read(&mut buffer)
            .map_err(|_| CacheRefusal("read file"))?;
        if count == 0 {
            break;
        }
        bytes = bytes
            .checked_add(count as u64)
            .ok_or(CacheRefusal("file overflow"))?;
        if bytes > expected.bytes {
            return Err(CacheRefusal("file grew"));
        }
        hash.update(&buffer[..count]);
        if let Some(file) = destination.as_mut() {
            file.write_all(&buffer[..count])
                .map_err(|_| CacheRefusal("write file"))?;
        }
    }
    if bytes != expected.bytes || hexadecimal(&hash.finalize()) != expected.digest {
        return Err(CacheRefusal("file digest"));
    }
    Ok(())
}
pub(super) fn restore(
    repo: &Path,
    cache: &Path,
    output: &Path,
    approved_manifest_sha256: &str,
) -> Result<CacheReceipt> {
    // A failed restore must never leave a previous success assertion live.
    match fs::remove_file(output.join(RECEIPT)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err(CacheRefusal("clear receipt")),
    }
    println!("cargo:rerun-if-changed={}", cache.display());
    let manifest_path = cache.join(MANIFEST);
    println!("cargo:rerun-if-changed={}", manifest_path.display());
    let extent = fs::symlink_metadata(&manifest_path)
        .map_err(|_| CacheRefusal("missing manifest"))?
        .len();
    if extent == 0 || extent > MAXIMUM_MANIFEST_BYTES as u64 {
        return Err(CacheRefusal("manifest extent"));
    }
    let mut file = open_regular(&manifest_path, extent)?;
    let mut bytes = Vec::with_capacity(extent as usize);
    Read::by_ref(&mut file)
        .take(extent + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CacheRefusal("read manifest"))?;
    if bytes.len() as u64 != extent || digest(&bytes) != approved_manifest_sha256 {
        return Err(CacheRefusal("unapproved manifest"));
    }
    let manifest = std::str::from_utf8(&bytes).map_err(|_| CacheRefusal("manifest encoding"))?;
    let mut lines = manifest.lines();
    if lines.next() != Some("conduit.language.checked-output-cache.v1") {
        return Err(CacheRefusal("manifest schema"));
    }
    let entries = lines.map(entry).collect::<Result<Vec<_>>>()?;
    if entries.is_empty() || entries.len() > MAXIMUM_FILES {
        return Err(CacheRefusal("manifest count"));
    }
    let mut output_files = 0usize;
    let mut output_bytes = 0u64;
    let mut inputs = 0usize;
    let mut provenance = 0usize;
    for (index, (kind, expected)) in entries.iter().enumerate() {
        if entries[..index]
            .iter()
            .any(|(old_kind, old)| old_kind == kind && old.path == expected.path)
        {
            return Err(CacheRefusal("duplicate manifest path"));
        }
        let base = match *kind {
            "input" => {
                inputs += 1;
                repo
            }
            "provenance" => {
                provenance += 1;
                cache
            }
            "output" => {
                output_files += 1;
                output_bytes = output_bytes
                    .checked_add(expected.bytes)
                    .ok_or(CacheRefusal("output overflow"))?;
                if output_bytes > MAXIMUM_OUTPUT_BYTES {
                    return Err(CacheRefusal("output capacity"));
                }
                cache
            }
            _ => return Err(CacheRefusal("manifest kind")),
        };
        println!(
            "cargo:rerun-if-changed={}",
            base.join(expected.path).display()
        );
        transfer(
            open_regular(&base.join(expected.path), expected.bytes)?,
            None,
            expected,
        )?;
    }
    if inputs == 0
        || provenance == 0
        || output_files == 0
        || !entries
            .iter()
            .any(|(kind, e)| *kind == "output" && e.path == "semantic_types.rs")
    {
        return Err(CacheRefusal("incomplete cache"));
    }
    // All source/checker/options and output digests pass before the first write.
    // Each copy is rehashed to close mutation between preflight and publication.
    fs::create_dir_all(output).map_err(|_| CacheRefusal("output directory"))?;
    for (_, expected) in entries.iter().filter(|(kind, _)| *kind == "output") {
        let temporary = output.join(format!(
            ".checked-cache-{}-{}",
            std::process::id(),
            expected.path
        ));
        let mut destination = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| CacheRefusal("temporary output"))?;
        let copied = transfer(
            open_regular(&cache.join(expected.path), expected.bytes)?,
            Some(&mut destination),
            expected,
        );
        drop(destination);
        if let Err(error) = copied {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        fs::rename(&temporary, output.join(expected.path))
            .map_err(|_| CacheRefusal("publish output"))?;
    }
    // Inputs may have changed during the complete copy. Never attest that mix.
    for (_, expected) in entries.iter().filter(|(kind, _)| *kind == "input") {
        transfer(
            open_regular(&repo.join(expected.path), expected.bytes)?,
            None,
            expected,
        )?;
    }
    fs::write(output.join(RECEIPT), format!(
        "checked output reuse v1\nmanifest-sha256 {approved_manifest_sha256}\nfiles {output_files}\nbytes {output_bytes}\nSource reused; not rechecked by this invocation\n"
    )).map_err(|_| CacheRefusal("write receipt"))?;
    Ok(CacheReceipt {
        output_files,
        output_bytes,
        manifest_sha256: approved_manifest_sha256.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture {
        root: PathBuf,
        manifest: String,
    }
    impl Fixture {
        fn new() -> Self {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("target/checked-output-cache-tests")
                .join(format!(
                    "{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
            fs::create_dir_all(root.parent().unwrap()).unwrap();
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("repo")).unwrap();
            fs::create_dir(root.join("cache")).unwrap();
            fs::write(root.join("repo/types.conduit"), b"original Source").unwrap();
            fs::write(root.join("cache/graft.json"), b"reviewed complete parity").unwrap();
            fs::write(
                root.join("cache/semantic_types.rs"),
                b"original + additive converter",
            )
            .unwrap();
            fs::write(root.join("cache/law.bin"), [1, 2, 3]).unwrap();
            let mut manifest = String::from("conduit.language.checked-output-cache.v1\n");
            for (kind, path, base) in [
                ("input", "types.conduit", "repo"),
                ("provenance", "graft.json", "cache"),
                ("output", "semantic_types.rs", "cache"),
                ("output", "law.bin", "cache"),
            ] {
                let bytes = fs::read(root.join(base).join(path)).unwrap();
                manifest.push_str(&format!(
                    "{kind} {} {} {path}\n",
                    digest(&bytes),
                    bytes.len()
                ));
            }
            fs::write(root.join("cache").join(MANIFEST), &manifest).unwrap();
            Self { root, manifest }
        }
        fn restore(&self, approved: &str) -> Result<CacheReceipt> {
            restore(
                &self.root.join("repo"),
                &self.root.join("cache"),
                &self.root.join("output"),
                approved,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
    #[test]
    fn complete_approved_cache_preserves_every_output_byte() {
        let f = Fixture::new();
        let receipt = f.restore(&digest(f.manifest.as_bytes())).unwrap();
        assert_eq!(receipt.output_files, 2);
        for name in ["semantic_types.rs", "law.bin"] {
            assert_eq!(
                fs::read(f.root.join("cache").join(name)).unwrap(),
                fs::read(f.root.join("output").join(name)).unwrap()
            );
        }
        assert!(
            fs::read_to_string(f.root.join("output/checked-output-cache-receipt.txt"))
                .unwrap()
                .contains("Source reused; not rechecked")
        );
    }
    #[test]
    fn foreign_manifest_source_or_law_refuses_before_output_allocation() {
        for changed in ["manifest", "source", "law"] {
            let f = Fixture::new();
            let approved = digest(f.manifest.as_bytes());
            match changed {
                "manifest" => fs::write(
                    f.root.join("cache").join(MANIFEST),
                    f.manifest.replace("types.conduit", "other.conduit"),
                )
                .unwrap(),
                "source" => {
                    fs::write(f.root.join("repo/types.conduit"), b"foreign! Source").unwrap()
                }
                "law" => fs::write(f.root.join("cache/law.bin"), [1, 9, 3]).unwrap(),
                _ => unreachable!(),
            }
            assert!(f.restore(&approved).is_err());
            assert!(!f.root.join("output").exists());
        }
    }
    #[test]
    fn refusal_clears_previous_success_receipt_and_reserves_receipt_name() {
        let f = Fixture::new();
        let approved = digest(f.manifest.as_bytes());
        f.restore(&approved).unwrap();
        fs::write(f.root.join("repo/types.conduit"), b"changed Source!").unwrap();
        assert!(f.restore(&approved).is_err());
        assert!(!f.root.join("output").join(RECEIPT).exists());
        let mut f = Fixture::new();
        f.manifest
            .push_str(&format!("output {} 0 {RECEIPT}\n", digest(b"")));
        fs::write(f.root.join("cache").join(MANIFEST), &f.manifest).unwrap();
        assert!(f.restore(&digest(f.manifest.as_bytes())).is_err());
        assert!(!f.root.join("output").exists());
    }
    #[test]
    fn duplicate_or_traversing_output_is_never_published() {
        for entry in [
            "output 0000000000000000000000000000000000000000000000000000000000000000 0 ../foreign\n",
            "output 0000000000000000000000000000000000000000000000000000000000000000 0 law.bin\n",
        ] {
            let mut f = Fixture::new();
            f.manifest.push_str(entry);
            fs::write(f.root.join("cache").join(MANIFEST), &f.manifest).unwrap();
            assert!(f.restore(&digest(f.manifest.as_bytes())).is_err());
            assert!(!f.root.join("output").exists());
        }
    }
}
