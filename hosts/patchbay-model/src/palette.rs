//! Authoritative, bounded discovery projection for reusable semantic Kinds.

use conduit_core::{ConfigurationValue, KindId, PortDescriptor};

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
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteError {
    CatalogTooLarge,
    QueryTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GearPalette {
    entries: Vec<PaletteEntry>,
}

impl GearPalette {
    /// Projects the supported executable nucleus, rather than maintaining a
    /// Patchbay-private list of Kind contracts.
    pub fn standard() -> Result<Self, PaletteError> {
        let contracts = conduit_std_catalog::supported_nucleus_contracts();
        if contracts.len() > MAX_PALETTE_ENTRIES {
            return Err(PaletteError::CatalogTooLarge);
        }
        let entries = contracts
            .into_iter()
            .map(|contract| PaletteEntry {
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
            })
            .collect();
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
        assert_eq!(palette.entries().len(), 10);
        assert_eq!(palette.search("").unwrap().len(), 10);
        assert_eq!(
            palette.search("uppercase").unwrap()[0].kind_id.as_str(),
            "text/upper"
        );
        assert_eq!(palette.search("value/count").unwrap().len(), 2);
        assert_eq!(palette.search("maximum-values").unwrap().len(), 3);
        assert_eq!(
            palette.search(&"x".repeat(MAX_PALETTE_QUERY_BYTES + 1)),
            Err(PaletteError::QueryTooLarge)
        );
    }
}
