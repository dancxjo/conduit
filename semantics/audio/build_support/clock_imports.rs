//! Narrow imports from the checked Data clock owner; no repeated declarations.
use conduit_core::StructuredInfoType;
use conduit_plot::rust_binding::ExternalNativeRustBinding;
use conduit_plot::StartupCatalog;

pub struct OwnerImports {
    pub types: Vec<StructuredInfoType>,
    pub catalog: StartupCatalog,
    identities: Vec<String>,
    paths: Vec<&'static str>,
}
impl OwnerImports {
    pub fn new() -> Self {
        let checked = conduit_data::clock_authoring::checked_types()
            .expect("Data clock owner metadata reconstructs exactly");
        let mut catalog = StartupCatalog::new();
        let mut types = Vec::new();
        let mut identities = Vec::new();
        let mut paths = Vec::new();
        for ty in checked {
            let path = match ty.name.as_str() {
                "ClockRelationQuality" => "conduit_data::ClockRelationQuality",
                "ClockRelation" => "conduit_data::ClockRelation",
                _ => panic!("Data clock authoring unexpectedly exports another root"),
            };
            catalog
                .insert_checked_native_type(ty.name.clone(), &ty)
                .expect("Data clock owner Type installs once");
            identities.push(ty.identity.as_str().to_owned());
            types.push(ty.value_type);
            paths.push(path);
        }
        assert_eq!(
            types.len(),
            2,
            "Data clock authoring exports exactly two roots"
        );
        Self {
            types,
            catalog,
            identities,
            paths,
        }
    }
    pub fn bindings(&self) -> Vec<ExternalNativeRustBinding<'_>> {
        self.identities
            .iter()
            .zip(&self.paths)
            // Quality is nested inside this external owner root, not a
            // separately generated Audio field. Keep its checked metadata in
            // the catalog without requesting an unused Rust binding.
            .filter(|(_, path)| **path == "conduit_data::ClockRelation")
            .map(|(identity, path)| ExternalNativeRustBinding {
                semantic_identity: identity,
                rust_type_path: path,
            })
            .collect()
    }
}
