//! Explicitly versioned content-capable rows, retaining the legacy plain stream.
use super::*;
// The first three continuation bytes cannot encode a legacy Postcard u16 node.
const MAGIC: &[u8; 7] = b"\xff\xff\xffRSC1";
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct FragmentResource<'a> {
    #[serde(borrow)]
    pub binding: PlainFragmentResource<'a>,
    pub content: Option<FragmentContent<'a>>,
}
impl<'a> core::ops::Deref for FragmentResource<'a> {
    type Target = PlainFragmentResource<'a>;
    fn deref(&self) -> &Self::Target {
        &self.binding
    }
}
impl FragmentResource<'_> {
    /// Allocation-free equality against the original admitted item at Start.
    /// This checks retained custody; it cannot admit an item or issue authority.
    pub fn matches_admitted_item(
        &self,
        item: &conduit_core::ResourceAdmissionItem,
        host: &str,
        boot: &str,
    ) -> bool {
        let requirement = &item.requirement;
        let binding = &item.binding;
        if binding.pool_id.as_str() != self.pool
            || binding.class_id.as_str() != self.class
            || binding.units != self.units
            || requirement.class_id.as_str() != self.class
            || requirement.units != self.units
            || requirement.protected_role.is_some()
            || requirement.compute.is_some()
            || binding.protected.is_some()
            || binding.compute.is_some()
        {
            return false;
        }
        match (self.content, &requirement.content, &binding.content) {
            (None, None, None) => true,
            (Some(expected), Some(required), Some(held)) => {
                expected.contract == FragmentContentContract::from_core(required)
                    && expected.contract == FragmentContentContract::from_core(&held.contract)
                    && held.owner_host.as_str() == host
                    && held.owner_boot.as_str() == boot
                    && held.base_id.as_str() == expected.base
                    && held.residence_profile.as_str() == expected.residence_profile
            }
            _ => false,
        }
    }

    pub fn append_to(&self, bytes: &mut Vec<u8>) -> Result<(), BodyFragmentDocumentRefusal> {
        if !self.binding.valid()
            || self.content.is_some_and(|c| !c.valid())
            || (!bytes.is_empty() && !bytes.starts_with(MAGIC))
        {
            return Err(BodyFragmentDocumentRefusal::Metadata);
        }
        if bytes.is_empty() {
            bytes.extend_from_slice(MAGIC);
        }
        bytes.extend(
            postcard::to_allocvec(self).map_err(|_| BodyFragmentDocumentRefusal::Metadata)?,
        );
        Ok(())
    }
}
#[derive(Clone)]
pub struct FragmentResources<'a> {
    remaining: &'a [u8],
    versioned: bool,
}
impl<'a> FragmentResources<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        let (remaining, versioned) = bytes
            .strip_prefix(MAGIC)
            .map_or((bytes, false), |b| (b, true));
        Self {
            remaining,
            versioned,
        }
    }
}
impl<'a> Iterator for FragmentResources<'a> {
    type Item = Result<FragmentResource<'a>, BodyFragmentDocumentRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let bytes = core::mem::take(&mut self.remaining);
        let decoded = if self.versioned {
            postcard::take_from_bytes::<FragmentResource<'a>>(bytes)
        } else {
            postcard::take_from_bytes::<PlainFragmentResource<'a>>(bytes).map(|(binding, rest)| {
                (
                    FragmentResource {
                        binding,
                        content: None,
                    },
                    rest,
                )
            })
        };
        Some(match decoded {
            Ok((row, rest))
                if row.binding.valid()
                    && row.content.is_none_or(|c| c.valid())
                    && rest.len() < bytes.len() =>
            {
                self.remaining = rest;
                Ok(row)
            }
            _ => Err(BodyFragmentDocumentRefusal::Metadata),
        })
    }
}
