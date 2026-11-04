//! Compose immutable images without publishing a partially initialized inventory.
use crate::{PreparedResidentExpression, ResidentExpressionRefusal as R};

/// Emit one owner for an explicitly selected finite collection of resident Backs.
/// This admits neither their storage nor a Host, work pool, reservation or Play.
pub fn generate_resident_library(
    images: &[PreparedResidentExpression],
    maximum_generated_bytes: u32,
) -> Result<String, R> {
    if images.is_empty() || images.len() > 64 {
        return Err(R::Bounds);
    }
    let mut code = String::new();
    for (index, image) in images.iter().enumerate() {
        if images[..index]
            .iter()
            .any(|prior| prior.kind_id() == image.kind_id())
        {
            return Err(R::Selection);
        }
        code.push_str(&format!(
            "pub mod entry_{index}{{\n{}\n}}\n",
            image.source()
        ));
        if code.len() as u64 > u64::from(maximum_generated_bytes) {
            return Err(R::Bounds);
        }
    }
    code.push_str("#[derive(Debug)]pub enum PreparationRefusal{\n");
    for index in 0..images.len() {
        code.push_str(&format!(
            "Entry{index}(entry_{index}::PreparationRefusal),\n"
        ));
    }
    code.push_str("}\nimpl PreparationRefusal{pub fn entry(&self)->usize{match self{\n");
    for index in 0..images.len() {
        code.push_str(&format!("Self::Entry{index}(_)=>{index},\n"));
    }
    code.push_str("}}}\n/// Retains every initialized Back. Metadata alone is not admission.\npub struct Initialized{\n");
    for index in 0..images.len() {
        code.push_str(&format!("entry_{index}:entry_{index}::Initialized,\n"));
    }
    code.push_str(
        "}\nimpl Initialized{\npub fn prepare()->Result<Self,PreparationRefusal>{Ok(Self{\n",
    );
    for index in 0..images.len() {
        code.push_str(&format!("entry_{index}:entry_{index}::Initialized::prepare().map_err(PreparationRefusal::Entry{index})?,\n"));
    }
    code.push_str("})}\n");
    code.push_str(&format!(
        "pub fn offers(&self)->[&[u8];{}]{{[\n",
        images.len()
    ));
    for index in 0..images.len() {
        code.push_str(&format!("self.entry_{index}.offer_json(),\n"));
    }
    code.push_str("]}\npub fn into_backs(self)->(\n");
    for index in 0..images.len() {
        code.push_str(&format!("entry_{index}::Back,\n"));
    }
    code.push_str("){(\n");
    for index in 0..images.len() {
        code.push_str(&format!("self.entry_{index}.into_back(),\n"));
    }
    code.push_str(")}\n}\n");
    if code.len() as u64 > u64::from(maximum_generated_bytes) {
        return Err(R::Bounds);
    }
    Ok(code)
}
