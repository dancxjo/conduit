//! Cumulative owner-capture limits, checked before registry mutation or cloning.
use super::NativeTypeFamily;
use crate::{Span, StartupCatalog, SyntaxCheckDiagnostic};

pub(super) const MAXIMUM_FAMILIES: usize = 128;
const MAXIMUM_DEPENDENCIES: usize = 4096;
const MAXIMUM_DEPENDENCY_BYTES: usize = 1024 * 1024;

pub(crate) fn validate(
    catalog: &StartupCatalog,
    extra: Option<(&str, &NativeTypeFamily)>,
    span: Span,
) -> Result<(), SyntaxCheckDiagnostic> {
    if catalog
        .native_families
        .len()
        .saturating_add(usize::from(extra.is_some()))
        > MAXIMUM_FAMILIES
    {
        return Err(super::super::diagnostic(
            span,
            "native family registry exceeds its 128-entry profile".into(),
        ));
    }
    let families = || {
        catalog
            .native_families
            .values()
            .chain(extra.iter().map(|(_, family)| *family))
    };
    super::super::generic::budget::validate_iter(
        families().flat_map(|family| family.templates.iter().chain(family.origins.values())),
    )?;
    let mut dependencies = 0usize;
    let mut bytes = extra.map_or(0, |(name, _)| name.len());
    bytes = catalog
        .native_families
        .keys()
        .fold(bytes, |sum, name| sum.saturating_add(name.len()));
    for family in families() {
        for dependency in &family.dependencies {
            dependencies = dependencies.saturating_add(1);
            if dependencies > MAXIMUM_DEPENDENCIES {
                return Err(super::super::diagnostic(
                    span,
                    "native family registry exceeds its dependency-count profile".into(),
                ));
            }
            bytes = bytes.saturating_add(dependency.name.len());
            let encoded = dependency.value_type.canonical_bytes().map_err(|refusal| {
                super::super::diagnostic(span, alloc::format!("invalid captured Type: {refusal:?}"))
            })?;
            bytes = bytes.saturating_add(encoded.len());
            for contract in &dependency.value_contracts {
                bytes = bytes
                    .saturating_add(contract.representation_path.len())
                    .saturating_add(contract.contract.identity_bytes().len());
            }
            for law in &dependency.invariants {
                let encoded = law.canonical_bytes().map_err(|refusal| {
                    super::super::diagnostic(
                        span,
                        alloc::format!("invalid captured law: {refusal:?}"),
                    )
                })?;
                bytes = bytes.saturating_add(encoded.len());
            }
            if bytes > MAXIMUM_DEPENDENCY_BYTES {
                return Err(super::super::diagnostic(
                    span,
                    "native family registry exceeds its finite dependency-byte profile".into(),
                ));
            }
        }
    }
    if bytes > MAXIMUM_DEPENDENCY_BYTES {
        return Err(super::super::diagnostic(
            span,
            "native family registry exceeds its finite lookup-byte profile".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> (crate::SyntaxDocument, StartupCatalog) {
        let parsed = crate::parse_syntax_document("type Vector<T, N: U16> = collection T = N\n");
        let (_, catalog) =
            crate::native_type::check_native_types(&parsed.types, &StartupCatalog::new()).unwrap();
        (parsed, catalog)
    }
    fn fill(catalog: &mut StartupCatalog, count: usize) {
        let (parsed, owner) = owner();
        for index in 0..count {
            super::super::install(
                catalog,
                &alloc::format!("example/families/Vector{index}"),
                &parsed.types[0],
                &parsed.types,
                &owner,
                [0; 32],
            )
            .unwrap();
        }
    }
    #[test]
    fn registry_capacity_refuses_before_mutation_and_keeps_collision_distinct() {
        let mut catalog = StartupCatalog::new();
        fill(&mut catalog, MAXIMUM_FAMILIES);
        validate(&catalog, None, owner().0.types[0].name.span).unwrap();
        let before = catalog.clone();
        let (parsed, owner) = owner();
        let failure = super::super::install(
            &mut catalog,
            "example/families/Extra",
            &parsed.types[0],
            &parsed.types,
            &owner,
            [0; 32],
        )
        .unwrap_err();
        assert!(failure.message.contains("128-entry"));
        assert_eq!(catalog, before);
        let collision = super::super::install(
            &mut catalog,
            "example/families/Vector0",
            &parsed.types[0],
            &parsed.types,
            &owner,
            [0; 32],
        )
        .unwrap_err();
        assert!(collision.message.contains("duplicate or ambiguous"));
        assert_eq!(catalog, before);
    }
    #[test]
    fn alias_capacity_is_checked_before_copying_the_owner_capture() {
        let mut catalog = StartupCatalog::new();
        fill(&mut catalog, MAXIMUM_FAMILIES - 1);
        let source = crate::parse_syntax_document(
            "with example/families/Vector0 as Samples\ntype Value = Samples<U8, 2>\n",
        );
        let admitted = crate::native_type::install_import_aliases(&source, &catalog).unwrap();
        assert_eq!(admitted.native_families.len(), MAXIMUM_FAMILIES);
        let failure = crate::native_type::install_import_aliases(&source, &admitted).unwrap_err();
        assert!(failure.message.contains("duplicate"));
        let other = crate::parse_syntax_document(
            "with example/families/Vector0 as More\ntype Value = More<U8, 2>\n",
        );
        let failure = crate::native_type::install_import_aliases(&other, &admitted).unwrap_err();
        assert!(failure.message.contains("128-entry"));
        assert_eq!(catalog.native_families.len(), MAXIMUM_FAMILIES - 1);
    }
    #[test]
    fn lookup_byte_pressure_refuses_without_installing_the_capture() {
        let (parsed, owner) = owner();
        let mut catalog = StartupCatalog::new();
        let path = "x".repeat(MAXIMUM_DEPENDENCY_BYTES + 1);
        let before = catalog.clone();
        let failure = super::super::install(
            &mut catalog,
            &path,
            &parsed.types[0],
            &parsed.types,
            &owner,
            [0; 32],
        )
        .unwrap_err();
        assert!(failure.message.contains("lookup-byte"));
        assert_eq!(catalog, before);
    }
}
