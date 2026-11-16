//! Optional finite software memory construction. This description allocates
//! nothing and grants no execution, evidence retirement or persistence authority.
use crate::{
    BaseInitializationContract, ConfigurationValue, HostConfiguration, HostMakePackage,
    ImplementationOffer, KindConfigurationField, KindConfigurationRule, MakeContribution,
    MakeExtension, PackageCatalogContribution,
};
pub const BASE_KIND: &str = "kernel/sign-archive";
pub const IMPLEMENTATION: &str = "kernel/affine-prefix-sign-archive@1";
pub fn initialization_contract() -> BaseInitializationContract {
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            count("maximum_retained_events", 1, u64::from(u32::MAX)),
            count("maximum_canonical_bytes", 1, u64::MAX),
            count("template_events", 1, u64::from(u16::MAX)),
            count("template_runs", 1, u64::from(u16::MAX)),
            count("remote_streams", 0, u64::from(u16::MAX)),
            count("maximum_archive_backing_bytes", 1, u64::MAX),
            count("maximum_transfer_overlap_bytes", 1, u64::MAX),
        ],
    }
}
/// Explicitly composed extension, never automatically installed on any target.
/// Target patterns describe code compatibility, not a prepared memory owner.
pub struct SignArchivePackage {
    targets: Vec<String>,
}
impl SignArchivePackage {
    pub fn for_targets(targets: Vec<String>) -> Result<Self, String> {
        if targets.is_empty()
            || targets.len() > 64
            || targets.iter().any(|p| p.is_empty() || p.len() > 128)
        {
            return Err("archive package requires finite explicit target patterns".into());
        }
        Ok(Self { targets })
    }
}
impl HostMakePackage for SignArchivePackage {
    fn contribution(&self) -> MakeContribution {
        MakeContribution::Extension(MakeExtension {
            package_id: "conduit-kernel-sign-archive@1".into(),
            package_revision: 1,
            compatible_target_patterns: self.targets.clone(),
            offers: vec![ImplementationOffer {
                base_kind: BASE_KIND.into(),
                implementation_id: IMPLEMENTATION.into(),
                implementation_revision: 1,
                target_patterns: self.targets.clone(),
                prerequisites: vec![],
                build_feature: None,
            }],
            catalog: PackageCatalogContribution {
                initialization_contracts: [(IMPLEMENTATION.into(), initialization_contract())]
                    .into_iter()
                    .collect(),
                ..Default::default()
            },
        })
    }
}
/// Logical quotas are separate from physical backing and simultaneous transfer
/// overlap. Neither physical ceiling changes the original Host heap grant.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeclaredSignArchiveBounds {
    pub maximum_retained_events: u32,
    pub maximum_canonical_bytes: u64,
    pub template_events: u16,
    pub template_runs: u16,
    pub remote_streams: u16,
    pub maximum_archive_backing_bytes: u64,
    pub maximum_transfer_overlap_bytes: u64,
}
impl DeclaredSignArchiveBounds {
    /// Parsed intent only. An unregistered implementation is not certified here.
    pub fn from_configuration(config: &HostConfiguration) -> Result<Option<Self>, String> {
        let mut bases = config.bases.iter().filter(|b| b.kind == BASE_KIND);
        let Some(base) = bases.next() else {
            return Ok(None);
        };
        if bases.next().is_some() {
            return Err("requires one archive Base".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("archive initialization required")?
            .checked_values(&initialization_contract())?;
        let value = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(n)) => Ok(*n),
            _ => Err(format!("missing archive bound {key}")),
        };
        let declared = Self {
            maximum_retained_events: value("maximum_retained_events")? as u32,
            maximum_canonical_bytes: value("maximum_canonical_bytes")?,
            template_events: value("template_events")? as u16,
            template_runs: value("template_runs")? as u16,
            remote_streams: value("remote_streams")? as u16,
            maximum_archive_backing_bytes: value("maximum_archive_backing_bytes")?,
            maximum_transfer_overlap_bytes: value("maximum_transfer_overlap_bytes")?,
        };
        if declared.maximum_archive_backing_bytes > config.limits.heap_arena_bytes
            || declared.maximum_transfer_overlap_bytes > config.limits.heap_arena_bytes
        {
            return Err("archive physical ceilings exceed original Host heap grant".into());
        }
        // No guessed target sizeof, logical-bytes-to-RAM comparison, or sum of
        // overlapping ceilings. Actual prepared owners must prove their overlap.
        Ok(Some(declared))
    }
}
#[cfg(test)]
mod tests;
