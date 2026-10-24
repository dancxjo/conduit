//! Concrete declaration replay after global mixed-material preflight.
use super::*;

pub(super) fn verify(
    material: &NativeLawMaterial,
    expected: &StructuredInfoType,
    depth: usize,
    budget: &mut Budget,
    context: &NativeLawContext<'_>,
) -> Result<NativeLawWitness, NativeLawCustodyRefusal> {
    budget.admit(&material.source, depth)?;
    if material.imports.len() > MAXIMUM_MODULES || material.declaration.len() > 4096 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    let syntax = crate::parse_syntax_document(&material.source);
    let required = required_imports(&syntax);
    let names = material
        .imports
        .iter()
        .map(|(name, _)| name)
        .chain(material.families.iter().map(|(name, _)| name))
        .collect::<alloc::collections::BTreeSet<_>>();
    if names.len() != material.imports.len() + material.families.len()
        || required.iter().ne(names.into_iter())
    {
        return Err(NativeLawCustodyRefusal::UnsupportedContext);
    }
    let mut catalog = StartupCatalog::new();
    let mut imports = Vec::new();
    let mut previous: Option<&str> = None;
    for (name, dependency) in &material.imports {
        if name.len() > 4096 || previous.is_some_and(|p| p >= name.as_str()) {
            return Err(NativeLawCustodyRefusal::DuplicateImport);
        }
        previous = Some(name);
        budget.charge(name.len())?;
        let witness = verify(
            dependency,
            &dependency.claimed.value_type,
            depth + 1,
            budget,
            context,
        )?;
        catalog
            .insert_checked_native_type(name.clone(), witness.checked_type())
            .map_err(|_| NativeLawCustodyRefusal::DuplicateImport)?;
        imports.push((name.clone(), witness));
    }
    let mut families = Vec::new();
    for (name, dependency) in &material.families {
        if name != &dependency.exported_path {
            return Err(NativeLawCustodyRefusal::MissingContext);
        }
        let witness = verify_family(dependency, context, depth + 1, budget)?;
        witness.install(name, &mut catalog)?;
        families.push((name.clone(), witness));
    }
    let checked = check_module(&material.source, &catalog)?;
    let actual = checked
        .iter()
        .find(|ty| ty.name == material.declaration)
        .ok_or(NativeLawCustodyRefusal::Declaration)?;
    if &actual.value_type != expected || actual.identity != material.claimed.identity {
        return Err(NativeLawCustodyRefusal::Type);
    }
    if actual != &material.claimed {
        return Err(NativeLawCustodyRefusal::Laws);
    }
    budget.claims(actual)?;
    Ok(NativeLawWitness {
        module: Shared::new(Module {
            source: material.source.clone(),
            imports,
            families,
        }),
        checked: actual.clone(),
    })
}
