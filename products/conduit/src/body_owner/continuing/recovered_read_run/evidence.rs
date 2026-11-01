//! Bounded observations of verified content; never an authority or input token.
use super::*;
impl RecoveredReadResult {
    pub(in super::super) fn evidence(&self, owner: &Owner) -> Result<serde_json::Value, String> {
        let state = owner
            .recovered_source_catalog
            .as_ref()
            .ok_or("verified content has no recovered Body State reference")?;
        let ad = owner.host.advertisement();
        if state.binding().body.as_str() != owner.session.evidence().body_id.as_str()
            || state.host_id() != &ad.host_id
            || state.boot_id() != &ad.boot_id
        {
            return Err("verified content belongs to another Body or Host Boot".into());
        }
        match self {
            Self::Catalog(contents) => {
                if contents.catalog_reference() != state.reference() {
                    return Err("verified catalog differs from recovered Body State".into());
                }
                let indexed = contents.references().iter().enumerate().map(|(index, reference)| {
                    Ok(serde_json::json!({"index":index,"reference_hex":reference_hex(reference)?,
                        "canonical_sha256":hex(&reference.identity.digest()),"canonical_bytes":reference.extent.bytes}))
                }).collect::<Result<Vec<_>, String>>()?;
                Ok(
                    serde_json::json!({"schema":"conduit.environment/recovered-catalog-evidence@1",
                    "catalog_reference_hex":reference_hex(contents.catalog_reference())?,
                    "catalog_canonical_sha256":hex(&contents.catalog_reference().identity.digest()),
                    "indexed_chunks":indexed}),
                )
            }
            Self::Chunk(chunk) => {
                let contents = owner
                    .recovered_source_catalog_contents
                    .as_ref()
                    .ok_or("verified chunk has no admitted catalog contents")?;
                if contents.catalog_reference() != state.reference()
                    || contents.references().get(chunk.index()) != Some(chunk.reference())
                {
                    return Err("verified chunk differs from current State-selected catalog".into());
                }
                let canonical = chunk
                    .snapshot()
                    .content()
                    .map_err(|e| format!("verified chunk evidence: {e:?}"))?;
                let digest: [u8; 32] = Sha256::digest(canonical).into();
                Ok(
                    serde_json::json!({"schema":"conduit.environment/recovered-source-chunk-evidence@1",
                    "catalog_reference_hex":reference_hex(contents.catalog_reference())?,
                    "index":chunk.index(),"reference_hex":reference_hex(chunk.reference())?,
                    "canonical_sha256":hex(&digest),"canonical_bytes":canonical.len(),
                    "records":chunk.snapshot().len(),"source_scope":chunk.snapshot().context()}),
                )
            }
        }
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn reference_hex(reference: &c::BoundedResourceRef) -> Result<String, String> {
    reference
        .encode()
        .map(|bytes| hex(&bytes))
        .map_err(|e| format!("verified reference evidence: {e:?}"))
}
