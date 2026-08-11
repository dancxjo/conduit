//! Authoritative, bounded discovery projection for reusable semantic Kinds.

use conduit_core::{ConfigurationValue, KindId, PortDescriptor};
pub use conduit_std_catalog::{PaletteCategory, PaletteIconKey};

pub const MAX_PALETTE_ENTRIES: usize = 64;
pub const MAX_PALETTE_QUERY_BYTES: usize = 96;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteConfigurationSummary {
    pub key: String,
    pub default_value: ConfigurationValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub kind_id: KindId,
    pub plain_name: String,
    pub summary: String,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub configuration: Vec<PaletteConfigurationSummary>,
    pub category: PaletteCategory,
    pub tags: &'static [&'static str],
    pub icon: PaletteIconKey,
}

impl PaletteEntry {
    fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let query = query.to_ascii_lowercase();
        self.plain_name.to_ascii_lowercase().contains(&query)
            || self.kind_id.as_str().to_ascii_lowercase().contains(&query)
            || self.summary.to_ascii_lowercase().contains(&query)
            || self.inputs.iter().chain(&self.outputs).any(|port| {
                port.port_id.as_str().to_ascii_lowercase().contains(&query)
                    || port
                        .value_kind
                        .as_str()
                        .to_ascii_lowercase()
                        .contains(&query)
            })
            || self
                .configuration
                .iter()
                .any(|field| field.key.to_ascii_lowercase().contains(&query))
            || self.category.label().to_ascii_lowercase().contains(&query)
            || self.tags.iter().any(|tag| tag.contains(&query))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    CatalogTooLarge,
    QueryTooLarge,
    MissingMetadata(KindId),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearPalette {
    entries: Vec<PaletteEntry>,
}

impl GearPalette {
    /// Projects the supported executable nucleus, rather than maintaining a
    /// Patchbay-private list of Kind contracts.
    pub fn standard() -> Result<Self, PaletteError> {
        let contracts = conduit_std_catalog::palette_contracts();
        if contracts.len() > MAX_PALETTE_ENTRIES {
            return Err(PaletteError::CatalogTooLarge);
        }
        let mut entries = Vec::with_capacity(contracts.len());
        for contract in contracts {
            let metadata = conduit_std_catalog::palette_metadata(&contract.kind_id)
                .ok_or_else(|| PaletteError::MissingMetadata(contract.kind_id.clone()))?;
            entries.push(PaletteEntry {
                kind_id: contract.kind_id,
                plain_name: contract.plain_name,
                summary: contract.summary,
                inputs: contract.inputs,
                outputs: contract.outputs,
                configuration: contract
                    .configuration
                    .into_iter()
                    .map(|field| PaletteConfigurationSummary {
                        key: field.key,
                        default_value: field.default_value,
                    })
                    .collect(),
                category: metadata.category,
                tags: metadata.tags,
                icon: metadata.icon,
            });
        }
        entries.sort_by(|left, right| {
            left.category
                .cmp(&right.category)
                .then_with(|| left.plain_name.cmp(&right.plain_name))
        });
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }

    pub fn find(&self, kind_id: &KindId) -> Option<&PaletteEntry> {
        self.entries.iter().find(|entry| &entry.kind_id == kind_id)
    }

    pub fn search(&self, query: &str) -> Result<Vec<&PaletteEntry>, PaletteError> {
        if query.len() > MAX_PALETTE_QUERY_BYTES {
            return Err(PaletteError::QueryTooLarge);
        }
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.matches(query))
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_palette_is_exact_bounded_and_searches_contract_truth() {
        let palette = GearPalette::standard().unwrap();
        assert_eq!(palette.entries().len(), 31);
        assert_eq!(palette.search("").unwrap().len(), 31);
        assert_eq!(
            palette.search("uppercase").unwrap()[0].kind_id.as_str(),
            "text/upper"
        );
        assert_eq!(palette.search("value/count").unwrap().len(), 2);
        assert_eq!(palette.search("maximum-values").unwrap().len(), 5);
        assert_eq!(
            palette.search("interval").unwrap()[0].kind_id.as_str(),
            "time/every"
        );
        assert_eq!(
            palette.search("files").unwrap()[0].kind_id.as_str(),
            "file/copy"
        );
        let keyboard = palette.search("keyboard").unwrap()[0];
        assert_eq!(keyboard.kind_id.as_str(), "input/keyboard");
        assert_eq!(keyboard.outputs[0].value_kind.as_str(), "input/key-event@1");
        assert!(palette
            .entries()
            .iter()
            .all(|entry| !entry.icon.is_fallback()));
        assert_eq!(
            palette.search(&"x".repeat(MAX_PALETTE_QUERY_BYTES + 1)),
            Err(PaletteError::QueryTooLarge)
        );
    }

    #[test]
    fn category_and_icon_are_ordered_presentation_metadata_not_kind_identity() {
        let palette = GearPalette::standard().unwrap();
        let categories = palette
            .entries()
            .iter()
            .map(|entry| entry.category)
            .collect::<Vec<_>>();
        assert!(categories.windows(2).all(|pair| pair[0] <= pair[1]));

        let mut decorated = palette.find(&KindId::from("time/tick")).unwrap().clone();
        let semantic_contract = (
            decorated.kind_id.clone(),
            decorated.inputs.clone(),
            decorated.outputs.clone(),
            decorated.configuration.clone(),
        );
        decorated.category = PaletteCategory::Files;
        decorated.icon = PaletteIconKey::GenericGear;
        assert_eq!(
            semantic_contract,
            (
                decorated.kind_id,
                decorated.inputs,
                decorated.outputs,
                decorated.configuration,
            )
        );
    }
}
