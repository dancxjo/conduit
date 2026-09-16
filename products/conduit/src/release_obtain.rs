//! Installed target-oriented release acquisition from an exact catalog.

use conduit_host_fabrication::{
    acquire_local_release_artifact, acquire_release_artifact_with, ReleaseCatalog,
    ReleaseCatalogRefusal, MAXIMUM_RELEASE_CATALOG_BYTES, MAXIMUM_RELEASE_MANIFEST_BYTES,
};
use conduit_std_host::hosted_http::release::{HostedReleaseClient, HostedReleaseFetchRefusal};
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
struct ObtainReceipt {
    schema: &'static str,
    catalog_id: String,
    catalog_generation: u64,
    target_id: String,
    package_id: String,
    output: String,
    builder_adapter: String,
    deployment_adapter: String,
    manifest_sha256: String,
    manifest_bytes: u64,
    manifest_path: String,
    from_cache: bool,
    body_bound: bool,
    carrier_realized: bool,
    boot_observed: bool,
}

pub(crate) fn run(
    target: &str,
    catalog_path: &Path,
    expected_catalog_id: &str,
    mirror_root: &Path,
    cache_directory: &Path,
    minimum_generation: u64,
) -> Result<(), String> {
    let receipt = obtain(
        target,
        catalog_path,
        expected_catalog_id,
        mirror_root,
        cache_directory,
        minimum_generation,
    )?;
    println!(
        "{}",
        serde_json::to_string(&receipt)
            .map_err(|error| format!("encode release obtain receipt: {error}"))?
    );
    Ok(())
}

fn obtain(
    target: &str,
    catalog_path: &Path,
    expected_catalog_id: &str,
    mirror_root: &Path,
    cache_directory: &Path,
    minimum_generation: u64,
) -> Result<ObtainReceipt, String> {
    let catalog_source = catalog_path.to_string_lossy();
    let mirror_source = mirror_root.to_string_lossy();
    if (catalog_source.contains("://") && !catalog_source.starts_with("https://"))
        || (mirror_source.contains("://") && !mirror_source.starts_with("https://"))
    {
        return Err("network release sources must use HTTPS".into());
    }
    let catalog_remote = catalog_source.starts_with("https://");
    let mirror_remote = mirror_source.starts_with("https://");
    if catalog_remote != mirror_remote {
        return Err("catalog and artifact mirror must both be HTTPS or both be local".into());
    }
    let client = HostedReleaseClient::default();
    let catalog = if catalog_remote {
        let bytes = client
            .get(&catalog_source, MAXIMUM_RELEASE_CATALOG_BYTES)
            .map_err(|error| format!("release catalog download refused: {error:?}"))?;
        ReleaseCatalog::open_bytes(&bytes, minimum_generation)
    } else {
        ReleaseCatalog::open_local(catalog_path, minimum_generation)
    }
    .map_err(|error| format!("release catalog refused: {error:?}"))?;
    if catalog.catalog_id != expected_catalog_id {
        return Err("release catalog identity differs from the installed release channel".into());
    }
    let entry = catalog
        .entries
        .iter()
        .find(|entry| entry.target_id == target)
        .ok_or_else(|| format!("reviewed target {target} is absent from this release catalog"))?;
    let resolved = if mirror_remote {
        let url = format!(
            "{}/{}",
            mirror_source.trim_end_matches('/'),
            entry.manifest.path
        );
        acquire_release_artifact_with(
            &entry.manifest,
            MAXIMUM_RELEASE_MANIFEST_BYTES,
            cache_directory,
            || {
                client
                    .get(&url, entry.manifest.bytes)
                    .map_err(|error| match error {
                        HostedReleaseFetchRefusal::BodyBound => {
                            ReleaseCatalogRefusal::ArtifactBound
                        }
                        _ => ReleaseCatalogRefusal::ArtifactUnavailable,
                    })
            },
        )
    } else {
        acquire_local_release_artifact(
            mirror_root,
            &entry.manifest,
            MAXIMUM_RELEASE_MANIFEST_BYTES,
            cache_directory,
        )
    }
    .map_err(|error| format!("release artifact refused: {error:?}"))?;
    Ok(ObtainReceipt {
        schema: "conduit.release/obtain-receipt@1",
        catalog_id: expected_catalog_id.into(),
        catalog_generation: catalog.generation,
        target_id: target.into(),
        package_id: entry.package_id.clone(),
        output: entry.output.clone(),
        builder_adapter: entry.builder_adapter.clone(),
        deployment_adapter: entry.deployment_adapter.clone(),
        manifest_sha256: resolved.sha256,
        manifest_bytes: resolved.bytes,
        manifest_path: resolved.path.display().to_string(),
        from_cache: resolved.from_cache,
        body_bound: false,
        carrier_realized: false,
        boot_observed: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_host_fabrication::{
        ReleaseArtifactDescriptor, ReleaseCatalogEntry, RELEASE_CATALOG_SCHEMA,
    };
    use sha2::{Digest, Sha256};
    use std::fs;

    #[derive(Serialize)]
    struct Identity<'a> {
        schema: &'a str,
        generation: u64,
        entries: &'a [ReleaseCatalogEntry],
    }

    fn digest(bytes: &[u8]) -> String {
        format!("sha256:{:x}", Sha256::digest(bytes))
    }

    #[test]
    fn installed_target_selection_fetches_only_exact_manifest_and_reuses_cache() {
        let root = std::env::temp_dir().join(format!("conduit-obtain-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let mirror = root.join("mirror");
        let cache = root.join("cache");
        fs::create_dir_all(&mirror).unwrap();
        let manifest = b"reviewed ConduitOS manifest";
        fs::write(mirror.join("conduitos.json"), manifest).unwrap();
        fs::write(mirror.join("unselected-secret.bin"), b"invitation secret").unwrap();
        let entries = vec![ReleaseCatalogEntry {
            target_id: "conduitos/x86_64/pc".into(),
            package_id: "conduit-host-conduitos@1".into(),
            output: "disk-image".into(),
            builder_adapter: "conduitos/build@1".into(),
            deployment_adapter: "conduitos/carriers@1".into(),
            manifest: ReleaseArtifactDescriptor {
                path: "conduitos.json".into(),
                bytes: manifest.len() as u64,
                sha256: digest(manifest),
            },
        }];
        let identity = digest(
            &serde_json::to_vec(&Identity {
                schema: RELEASE_CATALOG_SCHEMA,
                generation: 9,
                entries: &entries,
            })
            .unwrap(),
        );
        let catalog = ReleaseCatalog {
            schema: RELEASE_CATALOG_SCHEMA.into(),
            generation: 9,
            catalog_id: identity.clone(),
            entries,
        };
        let catalog_path = root.join("catalog.json");
        fs::write(&catalog_path, serde_json::to_vec(&catalog).unwrap()).unwrap();

        assert!(obtain(
            "conduitos/x86_64/pc",
            Path::new("http://releases.example/catalog.json"),
            &identity,
            Path::new("http://releases.example/artifacts"),
            &cache,
            8,
        )
        .unwrap_err()
        .contains("must use HTTPS"));

        assert!(obtain(
            "conduitos/x86_64/pc",
            &catalog_path,
            &format!("sha256:{}", "0".repeat(64)),
            &mirror,
            &cache,
            8,
        )
        .unwrap_err()
        .contains("installed release channel"));
        assert!(!cache.exists());

        let first = obtain(
            "conduitos/x86_64/pc",
            &catalog_path,
            &identity,
            &mirror,
            &cache,
            8,
        )
        .unwrap();
        assert!(!first.from_cache);
        assert!(!first.body_bound);
        assert!(!first.carrier_realized);
        assert!(!first.boot_observed);
        fs::remove_file(mirror.join("conduitos.json")).unwrap();
        let second = obtain(
            "conduitos/x86_64/pc",
            &catalog_path,
            &identity,
            &mirror,
            &cache,
            8,
        )
        .unwrap();
        assert!(second.from_cache);
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
