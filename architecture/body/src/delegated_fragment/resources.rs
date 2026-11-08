//! Exact plain resource bindings, borrowed until the existing owner admits them.
use super::*;
#[cfg(feature = "authenticated-admission")]
use conduit_core as c;

/// Public projection data only. Protected, compute-lane and content bindings
/// need their full contracts and are deliberately outside this plain profile.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PlainFragmentResource<'a> {
    pub node: u16,
    pub resource: u16,
    #[serde(borrow)]
    pub placement: &'a str,
    pub pool: &'a str,
    pub class: &'a str,
    pub units: u32,
}
impl PlainFragmentResource<'_> {
    /// Authoring encoding aid only; the receiver still checks full correspondence.
    pub fn append_to(&self, bytes: &mut Vec<u8>) -> Result<(), BodyFragmentDocumentRefusal> {
        if !self.valid() {
            return Err(BodyFragmentDocumentRefusal::Metadata);
        }
        bytes.extend(
            postcard::to_allocvec(self).map_err(|_| BodyFragmentDocumentRefusal::Metadata)?,
        );
        Ok(())
    }

    fn valid(&self) -> bool {
        self.units > 0
            && [self.placement, self.pool, self.class]
                .iter()
                .all(|id| !id.is_empty() && id.len() <= MAX_LIFECYCLE_ID_BYTES)
    }
}

/// Self-delimiting Postcard entries; no allocation or untrusted Vec length.
#[derive(Clone)]
pub struct PlainFragmentResources<'a> {
    remaining: &'a [u8],
}
impl<'a> PlainFragmentResources<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }
}
impl<'a> Iterator for PlainFragmentResources<'a> {
    type Item = Result<PlainFragmentResource<'a>, BodyFragmentDocumentRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let bytes = core::mem::take(&mut self.remaining);
        Some(
            match postcard::take_from_bytes::<PlainFragmentResource<'a>>(bytes) {
                Ok((item, rest)) if item.valid() && rest.len() < bytes.len() => {
                    self.remaining = rest;
                    Ok(item)
                }
                _ => Err(BodyFragmentDocumentRefusal::Metadata),
            },
        )
    }
}

#[cfg(feature = "authenticated-admission")]
mod admission;
#[cfg(feature = "authenticated-admission")]
pub use admission::{
    admit_delegated_plain_resources, admit_delegated_resources, reserve_delegated_plain_resources,
    reserve_delegated_resources, DelegatedResourceRefusal,
};
#[cfg(feature = "authenticated-admission")]
mod preparation_bound;
#[cfg(feature = "authenticated-admission")]
pub use preparation_bound::{
    delegated_plain_resource_preparation_bytes, delegated_resource_preparation_bytes,
};

mod content_wire;
pub use content_wire::{FragmentContent, FragmentContentContract};
mod projection;
pub use projection::{FragmentResource, FragmentResources};
