//! Versioned references to independently installed canonical plain resource rows.
//! Neither a digest nor a checked table is resource possession or admission.
use super::*;
const MAGIC: &[u8; 8] = b"\xff\xff\xffRSR01";
pub const PLAIN_RESOURCE_REFERENCE_BYTES: usize = 46;
const MAXIMUM_TABLE_ROWS: usize =
    conduit_core::MAXIMUM_RESOURCE_ADMISSIONS * conduit_core::MAXIMUM_BINDINGS_PER_ADMISSION;
const MAXIMUM_TABLE_BYTES: usize = MAXIMUM_TABLE_ROWS * (3 * MAX_LIFECYCLE_ID_BYTES + 64);
#[derive(Clone, Copy)]
pub struct InstalledPlainResourceTable {
    bytes: &'static [u8],
    reference: [u8; PLAIN_RESOURCE_REFERENCE_BYTES],
}
impl InstalledPlainResourceTable {
    /// Check the immutable installed material independently of any received
    /// reference. No allocation, request, reservation or approval is created.
    pub fn new(bytes: &'static [u8]) -> Result<Self, BodyFragmentDocumentRefusal> {
        Ok(Self {
            bytes,
            reference: plain_resource_table_reference(bytes)?,
        })
    }
    pub fn canonical_bytes(&self) -> &'static [u8] {
        self.bytes
    }
    pub fn reference(&self) -> [u8; PLAIN_RESOURCE_REFERENCE_BYTES] {
        self.reference
    }
    #[cfg(feature = "authenticated-admission")]
    pub(in crate::delegated_fragment) fn resolve<'a>(
        &self,
        mut metadata: BodyFragmentApprovalMetadata<'a>,
    ) -> Result<BodyFragmentApprovalMetadata<'a>, BodyFragmentDocumentRefusal> {
        if metadata.schema != 4 || metadata.resource_bindings != self.reference {
            return Err(BodyFragmentDocumentRefusal::Metadata);
        }
        // This is the logical approved view, deliberately still schema4. It
        // cannot be serialized as wire metadata by the ordinary encoder.
        metadata.resource_bindings = self.bytes;
        Ok(metadata)
    }
}
/// Authoring data only: exact canonical row bytes, count and domain-separated
/// digest. This fixed reference grants no authority and resolves no installation.
pub fn plain_resource_table_reference(
    bytes: &[u8],
) -> Result<[u8; PLAIN_RESOURCE_REFERENCE_BYTES], BodyFragmentDocumentRefusal> {
    use BodyFragmentDocumentRefusal as E;
    if bytes.len() > MAXIMUM_TABLE_BYTES {
        return Err(E::Bounds);
    }
    let length = u32::try_from(bytes.len()).map_err(|_| E::Bounds)?;
    let mut remaining = bytes;
    let mut count = 0usize;
    while !remaining.is_empty() {
        if count == MAXIMUM_TABLE_ROWS {
            return Err(E::Bounds);
        }
        let (row, rest) = postcard::take_from_bytes::<PlainFragmentResource<'_>>(remaining)
            .map_err(|_| E::Metadata)?;
        if !row.valid() || rest.len() >= remaining.len() {
            return Err(E::Metadata);
        }
        let consumed = remaining.len() - rest.len();
        if row.encoded_length()? != consumed {
            return Err(E::Metadata);
        }
        postcard::serialize_with_flavor(
            &row,
            Canonical {
                remaining: &remaining[..consumed],
            },
        )
        .map_err(|_| E::Metadata)?;
        remaining = rest;
        count += 1;
    }
    let count = u16::try_from(count).map_err(|_| E::Bounds)?;
    let mut hash = Sha256::new();
    hash.update(b"conduit.body/installed-plain-resources@1");
    hash.update(length.to_le_bytes());
    hash.update(count.to_le_bytes());
    hash.update(bytes);
    let mut reference = [0; PLAIN_RESOURCE_REFERENCE_BYTES];
    reference[..8].copy_from_slice(MAGIC);
    reference[8..40].copy_from_slice(&hash.finalize());
    reference[40..44].copy_from_slice(&length.to_le_bytes());
    reference[44..].copy_from_slice(&count.to_le_bytes());
    Ok(reference)
}
pub(crate) fn check_reference(bytes: &[u8]) -> bool {
    if bytes.len() != PLAIN_RESOURCE_REFERENCE_BYTES || !bytes.starts_with(MAGIC) {
        return false;
    }
    let length = u32::from_le_bytes(bytes[40..44].try_into().expect("checked width")) as usize;
    let count = u16::from_le_bytes(bytes[44..46].try_into().expect("checked width")) as usize;
    length <= MAXIMUM_TABLE_BYTES && count <= MAXIMUM_TABLE_ROWS && (length == 0) == (count == 0)
}
struct Canonical<'a> {
    remaining: &'a [u8],
}
impl postcard::ser_flavors::Flavor for Canonical<'_> {
    type Output = ();
    fn try_push(&mut self, byte: u8) -> postcard::Result<()> {
        if self.remaining.first() != Some(&byte) {
            return Err(postcard::Error::SerializeBufferFull);
        }
        self.remaining = &self.remaining[1..];
        Ok(())
    }
    fn finalize(self) -> postcard::Result<()> {
        if self.remaining.is_empty() {
            Ok(())
        } else {
            Err(postcard::Error::SerializeBufferFull)
        }
    }
}
