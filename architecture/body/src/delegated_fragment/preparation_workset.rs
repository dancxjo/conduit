//! One finite document for the complete local set, including selected Masks.
use super::*;
const MAGIC: &[u8; 8] = b"CBWSET01";
const MAXIMUM_FRAGMENTS: usize = conduit_core::MAXIMUM_RESOURCE_ADMISSIONS;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyWorksetDocumentRefusal {
    Bounds,
    Encoding,
    Fragment(BodyFragmentDocumentRefusal),
    IncompleteScope,
}

/// Encoding public data issues no signature, approval or reservation. Every
/// document must already come from ordinary checked Body-fragment preparation.
pub fn encode_body_workset_document(
    fragments: &[&[u8]],
    maximum_bytes: u32,
) -> Result<Vec<u8>, BodyWorksetDocumentRefusal> {
    use BodyWorksetDocumentRefusal as E;
    if fragments.is_empty() || fragments.len() > MAXIMUM_FRAGMENTS {
        return Err(E::Bounds);
    }
    let mut metadata = [None; MAXIMUM_FRAGMENTS];
    let mut total = 10usize;
    for (index, bytes) in fragments.iter().enumerate() {
        metadata[index] = Some(decode_body_fragment_document(bytes).map_err(E::Fragment)?.0);
        total = total
            .checked_add(4)
            .and_then(|n| n.checked_add(bytes.len()))
            .ok_or(E::Bounds)?;
    }
    if total as u64 > u64::from(maximum_bytes) {
        return Err(E::Bounds);
    }
    if !workset::matches_local_metadata(fragments.len(), |i| metadata[i].expect("checked prefix")) {
        return Err(E::IncompleteScope);
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(fragments.len() as u16).to_le_bytes());
    for fragment in fragments {
        let len = u32::try_from(fragment.len()).map_err(|_| E::Bounds)?;
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(fragment);
    }
    Ok(bytes)
}

#[cfg(feature = "authenticated-admission")]
struct WorksetDocuments<'a, const N: usize> {
    documents: [Option<&'a [u8]>; N],
    count: usize,
    declared_runtime_state_bytes: u32,
}
#[cfg(feature = "authenticated-admission")]
impl<'a, const N: usize> WorksetDocuments<'a, N> {
    fn decode(bytes: &'a [u8]) -> Result<Self, BodyWorksetDocumentRefusal> {
        use BodyWorksetDocumentRefusal as E;
        if N == 0 || N > MAXIMUM_FRAGMENTS || bytes.get(..8) != Some(MAGIC) {
            return Err(E::Encoding);
        }
        let count = usize::from(u16::from_le_bytes(
            bytes
                .get(8..10)
                .ok_or(E::Encoding)?
                .try_into()
                .map_err(|_| E::Encoding)?,
        ));
        if count == 0 || count > N {
            return Err(E::Bounds);
        }
        let mut result = Self {
            documents: [None; N],
            count,
            declared_runtime_state_bytes: 0,
        };
        let mut metadata = [None; N];
        let mut remaining = &bytes[10..];
        for (index, slot) in metadata.iter_mut().enumerate().take(count) {
            let length = u32::from_le_bytes(
                remaining
                    .get(..4)
                    .ok_or(E::Encoding)?
                    .try_into()
                    .map_err(|_| E::Encoding)?,
            );
            let length = usize::try_from(length).map_err(|_| E::Bounds)?;
            remaining = &remaining[4..];
            let document = remaining.get(..length).ok_or(E::Encoding)?;
            let (scope, projection) =
                decode_body_fragment_document(document).map_err(E::Fragment)?;
            *slot = Some(scope);
            // An early aggregate ceiling only. Each child's Core decoder still
            // checks this declaration against its exact structural minimum.
            let state = u16::from_le_bytes(
                projection
                    .get(12..14)
                    .ok_or(E::Encoding)?
                    .try_into()
                    .map_err(|_| E::Encoding)?,
            );
            result.declared_runtime_state_bytes = result
                .declared_runtime_state_bytes
                .checked_add(u32::from(state))
                .ok_or(E::Bounds)?;
            result.documents[index] = Some(document);
            remaining = &remaining[length..];
        }
        if !remaining.is_empty() {
            return Err(E::Encoding);
        }
        if !workset::matches_local_metadata(count, |i| metadata[i].expect("checked prefix")) {
            return Err(E::IncompleteScope);
        }
        Ok(result)
    }
    fn document(&self, index: usize) -> Option<&'a [u8]> {
        self.documents.get(index).copied().flatten()
    }
}

/// Borrowed inspection of one complete document set. This performs only the
/// existing bounded envelope/child-digest checks, not attribution, delegation,
/// native admission, carrier possession or permission to execute any child.
#[cfg(feature = "authenticated-admission")]
pub struct BodyWorksetDocumentView<'a, const N: usize> {
    inner: WorksetDocuments<'a, N>,
}
#[cfg(feature = "authenticated-admission")]
impl<'a, const N: usize> BodyWorksetDocumentView<'a, N> {
    pub fn decode(bytes: &'a [u8]) -> Result<Self, BodyWorksetDocumentRefusal> {
        Ok(Self {
            inner: WorksetDocuments::decode(bytes)?,
        })
    }
    pub fn len(&self) -> usize {
        self.inner.count
    }
    pub fn is_empty(&self) -> bool {
        self.inner.count == 0
    }
    pub fn document(&self, index: usize) -> Option<&'a [u8]> {
        self.inner.document(index)
    }
}

#[cfg(feature = "authenticated-admission")]
mod receiving;
#[cfg(feature = "authenticated-admission")]
pub use receiving::*;
