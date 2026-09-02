//! Exact finite browser-application package manifests.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const TEMPLATE_SCHEMA: &str = "conduit.browser/application-package-template@1";
const PACKAGE_SCHEMA: &str = "conduit.browser/application-package@1";
const MAXIMUM_RESOURCES: usize = 32;
const MAXIMUM_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_TOTAL_RESOURCE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct Template {
    schema: String,
    application_id: String,
    state_compatibility: StateCompatibility,
    resources: Vec<TemplateResource>,
}

#[derive(Clone, Deserialize, Serialize)]
struct StateCompatibility {
    identity: String,
    version: u32,
}

#[derive(Deserialize)]
struct TemplateResource {
    role: String,
    kind: String,
    path: String,
    maximum_bytes: usize,
    dependencies: Vec<ResourceDependency>,
}

#[derive(Clone, Deserialize, Serialize)]
struct ResourceDependency {
    role: String,
    specifier: String,
}

#[derive(Serialize)]
struct Manifest {
    schema: &'static str,
    application_id: String,
    package_digest: String,
    state_compatibility: StateCompatibility,
    resources: Vec<ManifestResource>,
}

#[derive(Serialize)]
struct ManifestResource {
    role: String,
    kind: String,
    path: String,
    maximum_bytes: usize,
    sha256: String,
    dependencies: Vec<ResourceDependency>,
}

/// Builds a package manifest whose identities cover the exact bytes returned
/// by `resource`. The resolver is the Host's finite inventory boundary.
pub fn build_manifest<'a>(
    template_bytes: &[u8],
    mut resource: impl FnMut(&str) -> Option<&'a [u8]>,
) -> Result<Vec<u8>, String> {
    let template: Template = serde_json::from_slice(template_bytes)
        .map_err(|error| format!("decode browser application template: {error}"))?;
    if template.schema != TEMPLATE_SCHEMA
        || template.application_id.is_empty()
        || template.state_compatibility.identity.is_empty()
        || template.state_compatibility.version == 0
        || template.resources.is_empty()
        || template.resources.len() > MAXIMUM_RESOURCES
    {
        return Err("browser application template is outside its admitted bounds".into());
    }
    let mut roles = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut total_maximum_bytes = 0_usize;
    let mut resources = Vec::with_capacity(template.resources.len());
    for declaration in template.resources {
        if declaration.role.is_empty()
            || declaration.path.is_empty()
            || !roles.insert(declaration.role.clone())
            || !paths.insert(declaration.path.clone())
            || declaration.maximum_bytes == 0
            || declaration.maximum_bytes > MAXIMUM_RESOURCE_BYTES
        {
            return Err("browser application resource declaration is invalid".into());
        }
        total_maximum_bytes = total_maximum_bytes
            .checked_add(declaration.maximum_bytes)
            .ok_or("browser application resource bounds overflow")?;
        let bytes = resource(&declaration.path).ok_or_else(|| {
            format!(
                "browser application resource {} is unknown",
                declaration.path
            )
        })?;
        if bytes.is_empty() || bytes.len() > declaration.maximum_bytes {
            return Err(format!(
                "browser application resource {} exceeds its admitted bound",
                declaration.role
            ));
        }
        resources.push(ManifestResource {
            role: declaration.role,
            kind: declaration.kind,
            path: declaration.path,
            maximum_bytes: declaration.maximum_bytes,
            sha256: digest(bytes),
            dependencies: declaration.dependencies,
        });
    }
    if total_maximum_bytes > MAXIMUM_TOTAL_RESOURCE_BYTES {
        return Err("browser application aggregate resource bound is exceeded".into());
    }
    for declaration in &resources {
        if declaration.dependencies.iter().any(|dependency| {
            !roles.contains(&dependency.role) || dependency.role == declaration.role
        }) {
            return Err(format!(
                "browser application resource {} has an invalid dependency",
                declaration.role
            ));
        }
    }
    let canonical = canonical(
        &template.application_id,
        &template.state_compatibility,
        &resources,
    );
    serde_json::to_vec(&Manifest {
        schema: PACKAGE_SCHEMA,
        application_id: template.application_id,
        package_digest: digest(canonical.as_bytes()),
        state_compatibility: template.state_compatibility,
        resources,
    })
    .map_err(|error| format!("encode browser application manifest: {error}"))
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical(
    application_id: &str,
    state: &StateCompatibility,
    resources: &[ManifestResource],
) -> String {
    let mut canonical = format!(
        "conduit.browser/application-package-content@1\napplication\0{application_id}\nstate\0{}\0{}\n",
        state.identity, state.version
    );
    for resource in resources {
        let dependencies = resource
            .dependencies
            .iter()
            .map(|dependency| format!("{}={}", dependency.role, dependency.specifier))
            .collect::<Vec<_>>()
            .join(",");
        canonical.push_str(&format!(
            "resource\0{}\0{}\0{}\0{}\0{}\0{}\n",
            resource.role,
            resource.kind,
            resource.path,
            resource.maximum_bytes,
            resource.sha256,
            dependencies
        ));
    }
    canonical
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &[u8] = br#"{
      "schema":"conduit.browser/application-package-template@1",
      "application_id":"conduit.application/test",
      "state_compatibility":{"identity":"test-state","version":1},
      "resources":[
        {"role":"application-module","kind":"module","path":"app.mjs","maximum_bytes":8,"dependencies":[{"role":"runtime","specifier":"./runtime.wasm"}]},
        {"role":"runtime","kind":"wasm","path":"runtime.wasm","maximum_bytes":8,"dependencies":[]}
      ]
    }"#;

    #[test]
    fn exact_bytes_determine_package_identity_but_not_state_identity() {
        let manifest = |runtime: &'static [u8]| {
            let bytes = build_manifest(VALID, |path| match path {
                "app.mjs" => Some(b"app"),
                "runtime.wasm" => Some(runtime),
                _ => None,
            })
            .unwrap();
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap()
        };
        let first = manifest(b"one");
        let second = manifest(b"two");
        assert_ne!(first["package_digest"], second["package_digest"]);
        assert_eq!(first["state_compatibility"]["identity"], "test-state");
        assert_ne!(
            first["state_compatibility"]["identity"],
            first["package_digest"]
        );
    }

    #[test]
    fn unknown_duplicate_oversized_and_invalid_dependency_refuse() {
        assert!(build_manifest(VALID, |_| None)
            .unwrap_err()
            .contains("unknown"));
        let duplicate = String::from_utf8(VALID.to_vec())
            .unwrap()
            .replace("\"runtime\"", "\"application-module\"");
        assert!(build_manifest(duplicate.as_bytes(), |_| Some(b"x"))
            .unwrap_err()
            .contains("invalid"));
        assert!(build_manifest(VALID, |path| match path {
            "app.mjs" => Some(&b"123456789"[..]),
            _ => Some(b"x"),
        })
        .unwrap_err()
        .contains("bound"));
        let invalid = String::from_utf8(VALID.to_vec())
            .unwrap()
            .replace("\"runtime\",\"specifier\"", "\"absent\",\"specifier\"");
        assert!(build_manifest(invalid.as_bytes(), |_| Some(b"x"))
            .unwrap_err()
            .contains("invalid dependency"));
    }
}
