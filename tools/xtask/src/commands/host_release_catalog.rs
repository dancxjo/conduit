use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path, PathBuf},
};

use conduit_host_fabrication::{
    ReleaseArtifactDescriptor, ReleaseCatalog, ReleaseCatalogEntry, MAXIMUM_RELEASE_CATALOG_BYTES,
    MAXIMUM_RELEASE_CATALOG_ENTRIES, MAXIMUM_RELEASE_MANIFEST_BYTES, RELEASE_CATALOG_SCHEMA,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAXIMUM_DIRECTORY_ENTRIES: usize = 256;

#[derive(Deserialize)]
struct ReleaseManifestIdentity {
    schema: String,
    target_id: String,
    fabrication_package_id: String,
    output: String,
    builder_adapter: String,
    deployment_adapter: Option<String>,
}

#[derive(Serialize)]
struct CatalogIdentity<'a> {
    schema: &'a str,
    generation: u64,
    entries: &'a [ReleaseCatalogEntry],
}

pub(crate) fn run(
    root: &Path,
    generation: u64,
    dry_run: bool,
    json: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if generation == 0 {
        return Err("release catalog generation must be greater than zero".into());
    }
    let mut paths = fs::read_dir(root)?
        .take(MAXIMUM_DIRECTORY_ENTRIES + 1)
        .collect::<Result<Vec<_>, _>>()?;
    if paths.len() > MAXIMUM_DIRECTORY_ENTRIES {
        return Err("release directory exceeds its finite entry bound".into());
    }
    paths.sort_by_key(|entry| entry.file_name());
    let mut entries = Vec::new();
    let mut targets = BTreeSet::new();
    for item in paths {
        let file_type = item.file_type()?;
        if !file_type.is_file()
            || item.path().extension().and_then(|value| value.to_str()) != Some("json")
        {
            continue;
        }
        let bytes = fs::read(item.path())?;
        if bytes.is_empty() || bytes.len() as u64 > MAXIMUM_RELEASE_MANIFEST_BYTES {
            return Err(format!(
                "release manifest {} violates its finite byte bound",
                item.path().display()
            )
            .into());
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            return Err(format!("release JSON {} is malformed", item.path().display()).into());
        };
        let Some(schema) = value.get("schema").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !schema.starts_with("conduit.release/") || value.get("target_id").is_none() {
            continue;
        }
        let manifest: ReleaseManifestIdentity = serde_json::from_value(value).map_err(|error| {
            format!(
                "release manifest {} lacks catalog identity: {error}",
                item.path().display()
            )
        })?;
        if manifest.schema.is_empty() {
            return Err("release manifest schema is empty".into());
        }
        let Some(deployment_adapter) = manifest.deployment_adapter else {
            continue;
        };
        if !targets.insert(manifest.target_id.clone()) {
            return Err(format!("duplicate release target {}", manifest.target_id).into());
        }
        let name = item.file_name().to_string_lossy().into_owned();
        if !bounded_relative_path(&name) {
            return Err(format!("release manifest path {name} is not safely relative").into());
        }
        entries.push(ReleaseCatalogEntry {
            target_id: manifest.target_id,
            package_id: manifest.fabrication_package_id,
            output: manifest.output,
            builder_adapter: manifest.builder_adapter,
            deployment_adapter,
            manifest: ReleaseArtifactDescriptor {
                path: name,
                bytes: bytes.len() as u64,
                sha256: sha256(&bytes),
            },
        });
    }
    entries.sort_by(|left, right| left.target_id.cmp(&right.target_id));
    if entries.is_empty() || entries.len() > MAXIMUM_RELEASE_CATALOG_ENTRIES {
        return Err("release catalog has no entries or exceeds its finite target bound".into());
    }
    let identity = serde_json::to_vec(&CatalogIdentity {
        schema: RELEASE_CATALOG_SCHEMA,
        generation,
        entries: &entries,
    })?;
    let catalog = ReleaseCatalog {
        schema: RELEASE_CATALOG_SCHEMA.into(),
        generation,
        catalog_id: sha256(&identity),
        entries,
    };
    let bytes = serde_json::to_vec_pretty(&catalog)?;
    if bytes.len() as u64 > MAXIMUM_RELEASE_CATALOG_BYTES {
        return Err("sealed release catalog exceeds its finite byte bound".into());
    }
    ReleaseCatalog::open_bytes(&bytes, generation - 1)
        .map_err(|error| format!("sealed release catalog refused validation: {error:?}"))?;
    let output = root.join("release-catalog.json");
    if output.exists() {
        return Err(format!("refusing to overwrite {}", output.display()).into());
    }
    if dry_run {
        if !quiet {
            println!(
                "would seal release catalog {} generation={} targets={}",
                output.display(),
                catalog.generation,
                catalog.entries.len()
            );
        }
        return Ok(());
    }
    fs::write(&output, bytes)?;
    if json {
        println!(
            "{}",
            serde_json::json!({
                "schema": "conduit.release/catalog-seal@1",
                "path": output,
                "catalog_id": catalog.catalog_id,
                "generation": catalog.generation,
                "targets": catalog.entries.len(),
            })
        );
    } else if !quiet {
        println!(
            "SEALED release catalog {} generation={} targets={}",
            output.display(),
            catalog.generation,
            catalog.entries.len()
        );
    }
    Ok(())
}

fn bounded_relative_path(value: &str) -> bool {
    let path = PathBuf::from(value);
    !value.is_empty()
        && value.len() <= 256
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "conduit-release-catalog-seal-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    fn manifest(target: &str, deployment: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "schema": "conduit.release/host-bundle@1",
            "target_id": target,
            "fabrication_package_id": "conduit-host-hosted@1",
            "output": "native-bundle",
            "builder_adapter": "conduit-host-hosted/build-native@1",
            "deployment_adapter": deployment,
            "files": [{"path":"host", "bytes":1, "sha256":format!("sha256:{}", "1".repeat(64))}],
        })
    }

    #[test]
    fn seals_sorted_exact_manifests_and_skips_non_deployable_releases() {
        let root = fixture();
        fs::write(
            root.join("z.json"),
            serde_json::to_vec(&manifest("std/x86_64/computer", Some("hosted/install@1"))).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("a.json"),
            serde_json::to_vec(&manifest("avr/promicro", None)).unwrap(),
        )
        .unwrap();
        fs::write(
            root.join("evidence.json"),
            br#"{"schema":"conduit.evidence/test@1"}"#,
        )
        .unwrap();

        run(&root, 7, false, false, true).unwrap();

        let bytes = fs::read(root.join("release-catalog.json")).unwrap();
        let catalog = ReleaseCatalog::open_bytes(&bytes, 6).unwrap();
        assert_eq!(catalog.entries.len(), 1);
        assert_eq!(catalog.entries[0].target_id, "std/x86_64/computer");
        assert_eq!(catalog.entries[0].manifest.path, "z.json");
        assert_eq!(
            catalog.entries[0].manifest.sha256,
            sha256(&fs::read(root.join("z.json")).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn refuses_duplicate_targets_and_overwrite() {
        let root = fixture();
        let bytes =
            serde_json::to_vec(&manifest("std/x86_64/computer", Some("hosted/install@1"))).unwrap();
        fs::write(root.join("one.json"), &bytes).unwrap();
        fs::write(root.join("two.json"), &bytes).unwrap();
        assert!(run(&root, 1, false, false, true)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        fs::remove_file(root.join("two.json")).unwrap();
        run(&root, 1, false, false, true).unwrap();
        assert!(run(&root, 2, false, false, true)
            .unwrap_err()
            .to_string()
            .contains("overwrite"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn dry_run_validates_without_writing_catalog_state() {
        let root = fixture();
        fs::write(
            root.join("host.json"),
            serde_json::to_vec(&manifest("std/x86_64/computer", Some("hosted/install@1"))).unwrap(),
        )
        .unwrap();
        run(&root, 3, true, false, true).unwrap();
        assert!(!root.join("release-catalog.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
