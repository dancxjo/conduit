use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::CatalogError;

pub const SCHEMA: &str = "conduit.std/supported-nucleus-inventory@1";
pub const MAXIMUM_ENTRIES: usize = 64;

#[derive(Clone, Serialize)]
pub struct InventoryEntry {
    pub kind_id: String,
    pub contract_revision: String,
    pub contract: Value,
    pub canonical_offer: Value,
}

#[derive(Serialize)]
struct DigestBasis<'a> {
    schema: &'static str,
    entries: &'a [InventoryEntry],
}

pub struct Inventory {
    pub entries: Vec<InventoryEntry>,
    pub digest: String,
}

pub fn catalog_contracts() -> Vec<conduit_semantic_catalog::StandardKindContract> {
    let mut contracts = conduit_semantic_catalog::supported_nucleus_contracts();
    contracts.extend(conduit_semantic_catalog::patchbay_presentation_contracts());
    contracts
}

pub fn catalog_offers() -> Vec<conduit_core::CapabilityOffer> {
    let mut offers = conduit_std_host::supported_nucleus_offers();
    offers.extend(conduit_std_offers::patchbay_presentation_offers());
    offers
}

pub fn derive() -> Result<Inventory, CatalogError> {
    let contracts = catalog_contracts();
    let offers = catalog_offers();
    if contracts.len() != offers.len() || contracts.len() > MAXIMUM_ENTRIES {
        return Err(CatalogError::new(
            "semantic-catalog-inventory-out-of-bounds",
            format!(
                "contracts={}, offers={}, maximum={MAXIMUM_ENTRIES}",
                contracts.len(),
                offers.len()
            ),
        ));
    }

    let mut entries = Vec::with_capacity(contracts.len());
    for (contract, offer) in contracts.into_iter().zip(offers) {
        if contract.kind_id != offer.kind_id
            || contract.inputs != offer.inputs
            || contract.outputs != offer.outputs
            || contract.limits != offer.limits
        {
            return Err(CatalogError::new(
                "semantic-catalog-contract-offer-mismatch",
                offer.kind_id.as_str(),
            ));
        }
        entries.push(InventoryEntry {
            kind_id: offer.kind_id.as_str().to_owned(),
            contract_revision: offer.kind_contract_revision.as_str().to_owned(),
            contract: serde_json::to_value(contract).map_err(CatalogError::encoding)?,
            canonical_offer: serde_json::to_value(offer).map_err(CatalogError::encoding)?,
        });
    }
    let digest = digest(&entries)?;
    Ok(Inventory { entries, digest })
}

pub fn digest(entries: &[InventoryEntry]) -> Result<String, CatalogError> {
    let bytes = serde_json::to_vec(&DigestBasis {
        schema: SCHEMA,
        entries,
    })
    .map_err(CatalogError::encoding)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}
