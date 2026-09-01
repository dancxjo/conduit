use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const TEMPLATE_SCHEMA: &str = "conduit.browser/application-package-template@1";
const PACKAGE_SCHEMA: &str = "conduit.browser/application-package@1";
const MAXIMUM_RESOURCES: usize = 32;
const MAXIMUM_RESOURCE_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_TOTAL_RESOURCE_BYTES: usize = 32 * 1024 * 1024;
const TEMPLATE: &[u8] = include_bytes!("../../assets/book.application.template.json");

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

pub(super) fn book_manifest(runtime: &[u8]) -> Result<Vec<u8>, String> {
    let template: Template = serde_json::from_slice(TEMPLATE)
        .map_err(|error| format!("decode Book application template: {error}"))?;
    if template.schema != TEMPLATE_SCHEMA
        || template.application_id.is_empty()
        || template.state_compatibility.identity.is_empty()
        || template.state_compatibility.version == 0
        || template.resources.is_empty()
        || template.resources.len() > MAXIMUM_RESOURCES
    {
        return Err("Book application template is outside its admitted bounds".into());
    }
    let mut roles = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut total_maximum_bytes = 0_usize;
    let mut resources = Vec::with_capacity(template.resources.len());
    for resource in template.resources {
        if !roles.insert(resource.role.clone())
            || !paths.insert(resource.path.clone())
            || resource.maximum_bytes == 0
            || resource.maximum_bytes > MAXIMUM_RESOURCE_BYTES
        {
            return Err("Book application resource declaration is invalid".into());
        }
        total_maximum_bytes = total_maximum_bytes
            .checked_add(resource.maximum_bytes)
            .ok_or("Book application resource bounds overflow")?;
        let bytes = resource_bytes(&resource.path, runtime)
            .ok_or_else(|| format!("Book application resource {} is unknown", resource.path))?;
        if bytes.is_empty() || bytes.len() > resource.maximum_bytes {
            return Err(format!(
                "Book application resource {} exceeds its admitted bound",
                resource.role
            ));
        }
        resources.push(ManifestResource {
            role: resource.role,
            kind: resource.kind,
            path: resource.path,
            maximum_bytes: resource.maximum_bytes,
            sha256: digest(bytes),
            dependencies: resource.dependencies,
        });
    }
    if total_maximum_bytes > MAXIMUM_TOTAL_RESOURCE_BYTES {
        return Err("Book application aggregate resource bound is exceeded".into());
    }
    for resource in &resources {
        if resource
            .dependencies
            .iter()
            .any(|dependency| !roles.contains(&dependency.role) || dependency.role == resource.role)
        {
            return Err(format!(
                "Book application resource {} has an invalid dependency",
                resource.role
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
    .map_err(|error| format!("encode Book application manifest: {error}"))
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

fn resource_bytes<'a>(path: &str, runtime: &'a [u8]) -> Option<&'a [u8]> {
    match path {
        "book.mjs" => Some(super::BOOK_SCRIPT),
        "browser-host-membership.mjs" => Some(super::HOST_MEMBERSHIP),
        "book-state.mjs" => Some(super::BOOK_STATE),
        "assets/flow.js" => Some(super::book_assets::FLOW),
        "assets/flow-scene.js" => Some(super::book_assets::FLOW_SCENE),
        "assets/flow-layout.js" => Some(super::book_assets::FLOW_LAYOUT),
        "assets/flow-faceplate.js" => Some(super::book_assets::FLOW_FACEPLATE),
        "assets/portable-navigation.js" => Some(super::book_assets::PORTABLE_NAVIGATION),
        "book.css" => Some(super::BOOK_STYLE),
        "assets/react-flow.css" => Some(super::book_assets::REACT_FLOW_STYLE),
        "assets/react.min.js" => Some(super::book_assets::REACT),
        "assets/react-dom.min.js" => Some(super::book_assets::REACT_DOM),
        "assets/react-flow.min.js" => Some(super::book_assets::REACT_FLOW),
        "chapter-1.md" => Some(super::BOOK_CHAPTER_ONE),
        "chapter-2.md" => Some(super::BOOK_CHAPTER_TWO),
        "chapter-3.md" => Some(super::BOOK_CHAPTER_THREE),
        "chapter-4.md" => Some(super::BOOK_CHAPTER_FOUR),
        "chapter-5.md" => Some(super::BOOK_CHAPTER_FIVE),
        "chapter-6.md" => Some(super::BOOK_CHAPTER_SIX),
        "chapter-8.md" => Some(super::BOOK_CHAPTER_EIGHT),
        "runtime.wasm" => Some(runtime),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_identity_covers_every_exact_resource_and_not_state_compatibility() {
        let first: serde_json::Value =
            serde_json::from_slice(&book_manifest(b"runtime-a").unwrap()).unwrap();
        let second: serde_json::Value =
            serde_json::from_slice(&book_manifest(b"runtime-b").unwrap()).unwrap();
        assert_ne!(first["package_digest"], second["package_digest"]);
        assert_eq!(first["resources"].as_array().unwrap().len(), 21);
        assert!(first["resources"]
            .as_array()
            .unwrap()
            .iter()
            .all(|resource| resource["sha256"].as_str().unwrap().starts_with("sha256:")));
        assert_eq!(
            first["state_compatibility"]["identity"],
            "conduit.application/book-reading-state"
        );
        assert_ne!(
            first["state_compatibility"]["identity"],
            first["package_digest"]
        );
    }
}
