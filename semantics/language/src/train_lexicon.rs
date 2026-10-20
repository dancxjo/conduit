//! Pinned TRAIN-only immutable lexical support. This is content/read admission,
//! not Source snapshot/currentness, inferred POS, model or calibration evidence.
use alloc::{boxed::Box, sync::Arc};
use conduit_core::{
    semantic_digest, AdmittedResourceAccess, AuthorityContractId, BoundedResourceRef,
    ResourceDereferenceRequirement, ResourceExtent, ResourceLifetime,
    ResourceReferenceAccessRefusal, ResourceReferenceBinding, ResourceSemanticIdentity,
    ResourceVersionIdentity,
};
#[path = "train_lexicon_format.rs"]
mod format;

pub const PROFILE: &str = "language/ewt-train-resource-lexicon/short4-possets@1";
pub const CONTENT_PROFILE: &str = "language/ewt-train-lexicon-posset-binary@1";
pub const IDENTITY_DOMAIN: &str = "language/ewt-train-lexicon-resource@1";
pub const READ_ACCESS: &str = "language/lexicon/read@1";
pub const READ_AUTHORITY: &str = "language/lexicon/read-immutable-train@1";
pub const SOURCE_COMMIT: &str = "b7711cce01cdd4f5fcc0a8199b8a50d951b16c0c";
pub const SOURCE_SHA256: &str = "d68e06122a702464c613076523d56740f047e5bbe89dd90ec32737e04d952143";
pub const RESOURCE_SHA256: &str =
    "05c48c7e2e02e68088b2e359808a6fde72a1713aa8b2c1f0bf025f4583e0467f";
pub const RESOURCE_BYTES: usize = 222_999;
pub const ENTRIES: usize = 19_652;
pub const MAXIMUM_SURFACE_BYTES: usize = 128;
pub const MAXIMUM_LOOKUP_COMPARISONS: usize = 16;
pub const POS_CODES: [&str; 17] = [
    "ADJ", "ADP", "ADV", "AUX", "CCONJ", "DET", "INTJ", "NOUN", "NUM", "PART", "PRON", "PROPN",
    "PUNCT", "SCONJ", "SYM", "VERB", "X",
];
pub const CONTENT_DIGEST: [u8; 32] = [
    0xcb, 0x3d, 0x2c, 0xb5, 0xb9, 0x5f, 0x0f, 0x61, 0x6d, 0x95, 0xfd, 0x66, 0x53, 0xbf, 0x4a, 0xbe,
    0xd2, 0x81, 0x6d, 0xe3, 0x55, 0xcf, 0x03, 0x58, 0x5c, 0x4a, 0xd1, 0xec, 0x97, 0x1c, 0x18, 0xa8,
];
pub(crate) const SOURCE_DIGEST: [u8; 32] = [
    0xd6, 0x8e, 0x06, 0x12, 0x2a, 0x70, 0x24, 0x64, 0xc6, 0x13, 0x07, 0x65, 0x23, 0xd5, 0x67, 0x40,
    0xf0, 0x47, 0xe5, 0xbb, 0xe8, 0x9d, 0xd9, 0x0e, 0xc3, 0x27, 0x37, 0xe0, 0x4d, 0x95, 0x21, 0x43,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrainLexiconRefusal {
    Reference,
    Content,
    Authority(ResourceReferenceAccessRefusal),
    Format,
    Bounds,
    Utf8,
    Ordering,
    Candidates,
    QueryBound,
}
/// Metadata only. Constructing this portable reference grants no read access.
pub fn pinned_train_lexicon_reference() -> BoundedResourceRef {
    BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest(CONTENT_DIGEST),
        content_profile: CONTENT_PROFILE.into(),
        access_class: READ_ACCESS.into(),
        extent: ResourceExtent {
            bytes: RESOURCE_BYTES as u64,
            items: Some(ENTRIES as u64),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest(CONTENT_DIGEST),
            expires_at: None,
        },
    }
}
/// No public constructor can substitute a decoded row or lookup index. Actual
/// bytes and the compact index remain immutable beside the exact read grant.
pub struct PreparedTrainLexicon {
    reference: BoundedResourceRef,
    access: AdmittedResourceAccess,
    bytes: Arc<[u8]>,
    offsets: Box<[u32]>,
}
impl PreparedTrainLexicon {
    pub fn admit(
        reference: BoundedResourceRef,
        binding: &ResourceReferenceBinding,
        bytes: Arc<[u8]>,
    ) -> Result<Self, TrainLexiconRefusal> {
        if reference != pinned_train_lexicon_reference() {
            return Err(TrainLexiconRefusal::Reference);
        }
        let access = ResourceDereferenceRequirement {
            content_profile: CONTENT_PROFILE.into(),
            access_class: READ_ACCESS.into(),
            authority_contract: AuthorityContractId::from(READ_AUTHORITY),
            maximum_bytes: RESOURCE_BYTES as u64,
            maximum_items: Some(ENTRIES as u64),
        }
        .admit(&reference, binding)
        .map_err(TrainLexiconRefusal::Authority)?;
        if bytes.len() != RESOURCE_BYTES
            || semantic_digest(IDENTITY_DOMAIN, &bytes) != CONTENT_DIGEST
        {
            return Err(TrainLexiconRefusal::Content);
        }
        let offsets = format::validate(&bytes, ENTRIES)?;
        Ok(Self {
            reference,
            access,
            bytes,
            offsets,
        })
    }
    pub fn reference(&self) -> &BoundedResourceRef {
        &self.reference
    }
    pub fn access(&self) -> &AdmittedResourceAccess {
        &self.access
    }
    pub fn canonical(&self) -> &[u8] {
        &self.bytes
    }
    /// Logical retained buffer/index bytes; excludes Arc/allocator metadata/RSS.
    pub fn logical_storage_bytes(&self) -> usize {
        self.bytes.len() + core::mem::size_of_val(self.offsets.as_ref())
    }
    /// Exact-case complete row or explicit absence. No suffix/unknown-word POS
    /// rule, truncation, candidate selection or lexical confidence occurs here.
    pub fn lookup(
        &self,
        surface: &str,
    ) -> Result<Option<TrainLexiconLookup<'_>>, TrainLexiconRefusal> {
        if surface.is_empty() || surface.len() > MAXIMUM_SURFACE_BYTES {
            return Err(TrainLexiconRefusal::QueryBound);
        }
        let (mut low, mut high, mut comparisons) = (0, self.offsets.len() - 1, 0);
        while low < high {
            if comparisons == MAXIMUM_LOOKUP_COMPARISONS {
                return Err(TrainLexiconRefusal::Bounds);
            }
            comparisons += 1;
            let ordinal = low + (high - low) / 2;
            let start = self.offsets[ordinal] as usize;
            let row = format::row(&self.bytes, start)?;
            match row.surface.as_bytes().cmp(surface.as_bytes()) {
                core::cmp::Ordering::Less => low = ordinal + 1,
                core::cmp::Ordering::Greater => high = ordinal,
                core::cmp::Ordering::Equal => {
                    return Ok(Some(TrainLexiconLookup {
                        owner: self,
                        ordinal,
                        start,
                        end: row.end,
                        surface: row.surface,
                        codes: row.codes,
                        comparisons,
                    }))
                }
            }
        }
        Ok(None)
    }
}
pub struct TrainLexiconLookup<'a> {
    owner: &'a PreparedTrainLexicon,
    ordinal: usize,
    start: usize,
    end: usize,
    surface: &'a str,
    codes: &'a [u8],
    comparisons: usize,
}
impl<'a> TrainLexiconLookup<'a> {
    pub fn owner(&self) -> &'a PreparedTrainLexicon {
        self.owner
    }
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }
    pub fn surface(&self) -> &'a str {
        self.surface
    }
    /// Whole ascending TRAIN POS set, never an inferred selected POS.
    pub fn pos_codes(&self) -> &'a [u8] {
        self.codes
    }
    pub fn row_canonical(&self) -> &'a [u8] {
        &self.owner.bytes[self.start..self.end]
    }
    pub fn comparisons(&self) -> usize {
        self.comparisons
    }
}
