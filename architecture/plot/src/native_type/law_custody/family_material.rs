//! Original checked-package preimages for generic families, never rewritten AST.
use super::*;
use crate::{CheckedPackageBundle, PackageExportCatalog, PackageMemberSource};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFamilyMember {
    pub path: String,
    pub source: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFamilyMaterial {
    pub manifest: String,
    pub members: Vec<NativeFamilyMember>,
    pub exported_path: String,
    pub imports: Vec<(String, NativeLawMaterial)>,
    pub families: Vec<(String, NativeFamilyMaterial)>,
}
/// Independently trusted context, just like the expected concrete Type.
/// Never obtain this value from the material being verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeFamilyAnchor {
    pub exported_path: String,
    pub package_content_digest: [u8; 32],
}
#[derive(Default)]
pub struct NativeLawContext<'a> {
    pub families: &'a [NativeFamilyAnchor],
}
impl NativeLawContext<'_> {
    pub(super) fn validate(&self) -> Result<(), NativeLawCustodyRefusal> {
        if self.families.len() > MAXIMUM_MODULES {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        let mut names = alloc::collections::BTreeSet::new();
        for anchor in self.families {
            if anchor.exported_path.len() > 4096 || !names.insert(anchor.exported_path.as_str()) {
                return Err(NativeLawCustodyRefusal::DuplicateImport);
            }
        }
        Ok(())
    }
    fn anchor(&self, path: &str) -> Result<&NativeFamilyAnchor, NativeLawCustodyRefusal> {
        self.families
            .iter()
            .find(|anchor| anchor.exported_path == path)
            .ok_or(NativeLawCustodyRefusal::MissingContext)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeFamilyBundle {
    sources: Shared<BundleSources>,
    digest: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct BundleSources {
    manifest: String,
    members: Vec<NativeFamilyMember>,
}
impl NativeFamilyBundle {
    /// Called only after the existing package factory validates the full bundle.
    pub(crate) fn capture(
        bundle: &CheckedPackageBundle,
        manifest: &str,
        members: &[PackageMemberSource<'_>],
    ) -> Option<Self> {
        let mut budget = Budget::default();
        budget.admit(manifest, 0).ok()?;
        if members.len() > crate::MAXIMUM_PACKAGE_MEMBERS {
            return None;
        }
        for member in members {
            budget.charge(member.path.len()).ok()?;
            budget.admit(member.source, 0).ok()?;
        }
        let mut members = members
            .iter()
            .map(|member| NativeFamilyMember {
                path: member.path.into(),
                source: member.source.into(),
            })
            .collect::<Vec<_>>();
        members.sort_by(|left, right| left.path.cmp(&right.path));
        Some(Self {
            sources: Shared::new(BundleSources {
                manifest: manifest.into(),
                members,
            }),
            digest: bundle.package.content_digest,
        })
    }
    fn replay(&self, base: &StartupCatalog) -> Result<StartupCatalog, NativeLawCustodyRefusal> {
        replay(
            &self.sources.manifest,
            &self.sources.members,
            self.digest,
            base,
        )
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NativeFamilyWitness {
    bundle: NativeFamilyBundle,
    anchor: NativeFamilyAnchor,
    imports: Vec<(String, NativeLawWitness)>,
    families: Vec<(String, NativeFamilyWitness)>,
}
impl NativeFamilyWitness {
    pub(crate) fn capture(
        bundle: &NativeFamilyBundle,
        path: &str,
        base: &StartupCatalog,
    ) -> Option<Self> {
        let documents = bundle
            .sources
            .members
            .iter()
            .map(|member| crate::parse_syntax_document(&member.source))
            .collect::<Vec<_>>();
        let source = root_source::merge_documents(&documents).ok()?;
        let syntax = crate::parse_syntax_document(&source);
        let mut imports = Vec::new();
        let mut families = Vec::new();
        for name in required_imports(&syntax) {
            if let Some(witness) = base.native_law_witness(&name) {
                imports.push((name, witness));
            } else {
                families.push((
                    name.clone(),
                    base.native_families.get(&name)?.source_custody.as_ref()?,
                ));
            }
        }
        let mut budget = Budget::default();
        admit_sources(
            &bundle.sources.manifest,
            &bundle.sources.members,
            path,
            0,
            &mut budget,
        )
        .ok()?;
        for (name, witness) in &imports {
            budget.charge(name.len()).ok()?;
            admit_witness(witness, 1, &mut budget).ok()?;
        }
        for (name, witness) in &families {
            budget.charge(name.len()).ok()?;
            witness.admit(1, &mut budget).ok()?;
        }
        let mut replay_base = StartupCatalog::new();
        for (name, witness) in &imports {
            replay_base
                .insert_checked_native_type(name.clone(), witness.checked_type())
                .ok()?;
        }
        for (name, witness) in &families {
            witness.install(name, &mut replay_base).ok()?;
        }
        let checked = bundle.replay(&replay_base).ok()?;
        checked.native_families.get(path)?;
        Some(Self {
            bundle: bundle.clone(),
            anchor: NativeFamilyAnchor {
                exported_path: path.into(),
                package_content_digest: bundle.digest,
            },
            imports: imports
                .into_iter()
                .map(|(name, witness)| (name, witness.clone()))
                .collect(),
            families: families
                .into_iter()
                .map(|(name, witness)| (name, witness.clone()))
                .collect(),
        })
    }
    pub(super) fn admit(
        &self,
        depth: usize,
        budget: &mut Budget,
    ) -> Result<(), NativeLawCustodyRefusal> {
        admit_sources(
            &self.bundle.sources.manifest,
            &self.bundle.sources.members,
            &self.anchor.exported_path,
            depth,
            budget,
        )?;
        for (name, witness) in &self.imports {
            budget.charge(name.len())?;
            admit_witness(witness, depth + 1, budget)?;
        }
        for (name, witness) in &self.families {
            budget.charge(name.len())?;
            witness.admit(depth + 1, budget)?;
        }
        Ok(())
    }
    pub(crate) fn bounds(&self) -> Result<(usize, usize), NativeLawCustodyRefusal> {
        let mut budget = Budget::default();
        self.admit(0, &mut budget)?;
        Ok((budget.modules, budget.bytes))
    }
    pub(super) fn material(&self) -> NativeFamilyMaterial {
        NativeFamilyMaterial {
            manifest: self.bundle.sources.manifest.clone(),
            members: self.bundle.sources.members.clone(),
            exported_path: self.anchor.exported_path.clone(),
            imports: self
                .imports
                .iter()
                .map(|(name, witness)| (name.clone(), expand_material(witness)))
                .collect(),
            families: self
                .families
                .iter()
                .map(|(name, witness)| (name.clone(), witness.material()))
                .collect(),
        }
    }
    pub(super) fn anchors(
        &self,
        values: &mut BTreeMap<String, NativeFamilyAnchor>,
    ) -> Result<(), NativeLawCustodyRefusal> {
        if values
            .get(&self.anchor.exported_path)
            .is_some_and(|prior| prior != &self.anchor)
        {
            return Err(NativeLawCustodyRefusal::MissingContext);
        }
        values.insert(self.anchor.exported_path.clone(), self.anchor.clone());
        for (_, witness) in &self.imports {
            witness.collect_anchors(values)?;
        }
        for (_, witness) in &self.families {
            witness.anchors(values)?;
        }
        Ok(())
    }
    pub(super) fn install(
        &self,
        name: &str,
        base: &mut StartupCatalog,
    ) -> Result<(), NativeLawCustodyRefusal> {
        if name != self.anchor.exported_path {
            return Err(NativeLawCustodyRefusal::MissingContext);
        }
        if let Some(prior) = base.native_families.get(name) {
            return if prior.source_custody.as_ref() == Some(self) {
                Ok(())
            } else {
                Err(NativeLawCustodyRefusal::DuplicateImport)
            };
        }
        let mut dependencies = StartupCatalog::new();
        for (name, witness) in &self.imports {
            dependencies
                .insert_checked_native_type(name.clone(), witness.checked_type())
                .map_err(|_| NativeLawCustodyRefusal::DuplicateImport)?;
        }
        for (name, witness) in &self.families {
            witness.install(name, &mut dependencies)?;
        }
        let mut replayed = self.bundle.replay(&dependencies)?;
        let mut family = replayed
            .native_families
            .remove(name)
            .ok_or(NativeLawCustodyRefusal::Declaration)?;
        family.source_custody = Some(self.clone());
        super::super::family::custody_budget::retain_optional(base, &mut family);
        super::super::family::budget::validate(
            base,
            Some((name, &family)),
            family.templates[0].span,
        )
        .map_err(|_| NativeLawCustodyRefusal::Capacity)?;
        base.native_families.insert(name.into(), family);
        Ok(())
    }
}

pub(super) fn verify_family(
    material: &NativeFamilyMaterial,
    context: &NativeLawContext<'_>,
    depth: usize,
    budget: &mut Budget,
) -> Result<NativeFamilyWitness, NativeLawCustodyRefusal> {
    let anchor = context.anchor(&material.exported_path)?;
    let documents = material
        .members
        .iter()
        .map(|member| crate::parse_syntax_document(&member.source))
        .collect::<Vec<_>>();
    let source = root_source::merge_documents(&documents)?;
    let syntax = crate::parse_syntax_document(&source);
    let names = material
        .imports
        .iter()
        .map(|(name, _)| name)
        .chain(material.families.iter().map(|(name, _)| name))
        .collect::<alloc::collections::BTreeSet<_>>();
    if names.len() != material.imports.len() + material.families.len()
        || required_imports(&syntax).iter().ne(names)
    {
        return Err(NativeLawCustodyRefusal::UnsupportedContext);
    }

    let mut imports = Vec::new();
    let mut families = Vec::new();
    let mut base = StartupCatalog::new();
    for (name, child) in &material.imports {
        let witness = verify(child, &child.claimed.value_type, depth + 1, budget, context)?;
        base.insert_checked_native_type(name.clone(), witness.checked_type())
            .map_err(|_| NativeLawCustodyRefusal::DuplicateImport)?;
        imports.push((name.clone(), witness));
    }
    for (name, child) in &material.families {
        if name != &child.exported_path {
            return Err(NativeLawCustodyRefusal::MissingContext);
        }
        let witness = verify_family(child, context, depth + 1, budget)?;
        witness.install(name, &mut base)?;
        families.push((name.clone(), witness));
    }
    // Actual original bundle checking, with the independent digest anchor.
    let checked = replay(
        &material.manifest,
        &material.members,
        anchor.package_content_digest,
        &base,
    )?;
    checked
        .native_families
        .get(&material.exported_path)
        .ok_or(NativeLawCustodyRefusal::Declaration)?;
    let parsed = crate::parse_syntax_document(&material.manifest);
    let sources = material
        .members
        .iter()
        .map(|member| PackageMemberSource {
            path: &member.path,
            source: &member.source,
        })
        .collect::<Vec<_>>();
    let bundle =
        CheckedPackageBundle::from_sources(&material.manifest, &parsed.packages[0], &sources)
            .map_err(|_| NativeLawCustodyRefusal::Source)?;
    let retained = NativeFamilyBundle::capture(&bundle, &material.manifest, &sources)
        .ok_or(NativeLawCustodyRefusal::Capacity)?;
    Ok(NativeFamilyWitness {
        bundle: retained,
        anchor: anchor.clone(),
        imports,
        families,
    })
}
fn replay(
    manifest: &str,
    members: &[NativeFamilyMember],
    digest: [u8; 32],
    base: &StartupCatalog,
) -> Result<StartupCatalog, NativeLawCustodyRefusal> {
    let parsed = crate::parse_syntax_document(manifest);
    if !parsed.diagnostics.is_empty() || parsed.packages.len() != 1 {
        return Err(NativeLawCustodyRefusal::Source);
    }
    let sources = members
        .iter()
        .map(|member| PackageMemberSource {
            path: &member.path,
            source: &member.source,
        })
        .collect::<Vec<_>>();
    let bundle = CheckedPackageBundle::from_sources(manifest, &parsed.packages[0], &sources)
        .map_err(|_| NativeLawCustodyRefusal::Source)?;
    if bundle.package.content_digest != digest {
        return Err(NativeLawCustodyRefusal::MissingContext);
    }
    bundle
        .validate_against(manifest, &parsed.packages[0], &sources)
        .map_err(|_| NativeLawCustodyRefusal::Source)?;
    let exports =
        PackageExportCatalog::from_bundle(&bundle, manifest, &parsed.packages[0], &sources)
            .map_err(|_| NativeLawCustodyRefusal::Source)?;
    let mut staged = base.clone();
    exports
        .install_shipped_types_for_custody(&mut staged)
        .map_err(|_| NativeLawCustodyRefusal::Source)?;
    Ok(staged)
}
pub(super) fn admit_sources(
    manifest: &str,
    members: &[NativeFamilyMember],
    path: &str,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), NativeLawCustodyRefusal> {
    budget.admit(manifest, depth)?;
    if members.len() > crate::MAXIMUM_PACKAGE_MEMBERS || path.len() > 4096 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    budget.charge(path.len())?;
    for member in members {
        if member.path.len() > 4096 {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        budget.charge(member.path.len())?;
        budget.admit(&member.source, depth)?;
    }
    Ok(())
}
