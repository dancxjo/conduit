//! Caller-owned final documents, sharing the ordinary canonical assembler.
use super::*;
use core::ops::Range;

pub(super) trait Destination {
    fn len(&self) -> usize;
    fn document(&self, index: usize) -> Option<&[u8]>;
    fn retained_bytes(&self) -> Option<usize>;
    fn maximum_document_bytes(&self, available: usize) -> usize;
    fn append(
        &mut self,
        metadata: &BodyFragmentApprovalMetadata<'_>,
        projection: &[u8],
        maximum: u32,
    ) -> Result<usize, ContinuationScopeRefusal>;
}
impl Destination for ContinuationCandidateDocuments {
    fn len(&self) -> usize {
        self.documents.len()
    }
    fn document(&self, index: usize) -> Option<&[u8]> {
        self.documents.get(index).map(Vec::as_slice)
    }
    fn retained_bytes(&self) -> Option<usize> {
        self.retained_backing_bytes()
    }
    fn maximum_document_bytes(&self, available: usize) -> usize {
        available
    }
    fn append(
        &mut self,
        metadata: &BodyFragmentApprovalMetadata<'_>,
        projection: &[u8],
        maximum: u32,
    ) -> Result<usize, ContinuationScopeRefusal> {
        let document = encode_body_fragment_document(metadata, projection, maximum)
            .map_err(|_| ContinuationScopeRefusal::Bounds)?;
        let length = document.len();
        self.documents.push(document);
        Ok(length)
    }
}
/// Bounded offsets into the SAME caller-owned immutable document storage.
/// These are descriptive ranges, not an approval, command, or execution grant.
/// On any preparation refusal no ranges are returned; partially written bytes
/// must not be treated as a prepared workset.
#[derive(Clone, Copy, Debug)]
pub struct ContinuationCandidateRanges {
    ranges: [[usize; 2]; crate::MAX_BODY_PLOTS],
    count: usize,
    used: usize,
}
impl ContinuationCandidateRanges {
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn used_bytes(&self) -> usize {
        self.used
    }
    pub fn range(&self, index: usize) -> Option<Range<usize>> {
        let [start, end] = *self.ranges.get(index).filter(|_| index < self.count)?;
        Some(start..end)
    }
    pub fn document<'a>(&self, bytes: &'a [u8], index: usize) -> Option<&'a [u8]> {
        if bytes.len() < self.used {
            return None;
        }
        bytes.get(self.range(index)?)
    }
}
struct Caller<'a> {
    bytes: &'a mut [u8],
    result: ContinuationCandidateRanges,
}
impl Destination for Caller<'_> {
    fn len(&self) -> usize {
        self.result.len()
    }
    fn document(&self, index: usize) -> Option<&[u8]> {
        self.result.document(self.bytes, index)
    }
    fn retained_bytes(&self) -> Option<usize> {
        Some(self.bytes.len())
    }
    fn maximum_document_bytes(&self, _: usize) -> usize {
        self.bytes.len() - self.result.used
    }
    fn append(
        &mut self,
        metadata: &BodyFragmentApprovalMetadata<'_>,
        projection: &[u8],
        maximum: u32,
    ) -> Result<usize, ContinuationScopeRefusal> {
        use ContinuationScopeRefusal as E;
        metadata.check().map_err(|_| E::Scope)?;
        if projection.len() as u64 != u64::from(metadata.projection_length)
            || Sha256::digest(projection).as_slice() != metadata.projection_digest
        {
            return Err(E::Scope);
        }
        let header = postcard::experimental::serialized_size(metadata).map_err(|_| E::Bounds)?;
        let total = header.checked_add(projection.len()).ok_or(E::Bounds)?;
        let start = self.result.used;
        let end = start.checked_add(total).ok_or(E::Bounds)?;
        if total as u64 > u64::from(maximum)
            || end > self.bytes.len()
            || self.result.count == crate::MAX_BODY_PLOTS
        {
            return Err(E::Bounds);
        }
        postcard::to_slice(metadata, &mut self.bytes[start..start + header])
            .map_err(|_| E::Scope)?;
        self.bytes[start + header..end].copy_from_slice(projection);
        self.result.ranges[self.result.count] = [start, end];
        self.result.count += 1;
        self.result.used = end;
        Ok(total)
    }
}
/// Final documents use caller storage; no final document heap allocation is
/// retained. Scratch must fit the largest template/projection. Both full caller
/// capacities plus every temporary Vec capacity are charged to the existing
/// signed runtime/preparation ceiling; inline values and allocator overhead
/// remain separately funded. Input/output/scratch must be disjoint.
///
/// The same complete safe verifier runs before ranges are returned. Root still
/// borrows those immutable documents into FreshContinuationScope and performs
/// ordinary full admission/one-use Start; these offsets provide no authority.
#[allow(clippy::too_many_arguments)]
pub fn build_continuation_candidate_into<const PORTS: usize>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[PlainFragmentResource<'_>]],
    output: &mut [u8],
    scratch: &mut [u8],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateRanges, ContinuationScopeRefusal> {
    into::<PORTS, _>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        resources,
        output,
        scratch,
        maximum_preparation_bytes,
    )
}
/// Content-capable counterpart, preserving legacy plain encoding per fragment.
#[allow(clippy::too_many_arguments)]
pub fn build_continuation_candidate_with_resources_into<const PORTS: usize>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[FragmentResource<'_>]],
    output: &mut [u8],
    scratch: &mut [u8],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateRanges, ContinuationScopeRefusal> {
    into::<PORTS, _>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        resources,
        output,
        scratch,
        maximum_preparation_bytes,
    )
}
#[allow(clippy::too_many_arguments)]
fn into<const PORTS: usize, R: CandidateResource>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[R]],
    output: &mut [u8],
    scratch: &mut [u8],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateRanges, ContinuationScopeRefusal> {
    use ContinuationScopeRefusal as E;
    let budget = checked_budget(
        intent,
        bundle,
        full,
        pins,
        entries.len(),
        resources.len(),
        maximum_preparation_bytes,
    )?;
    let held = output.len().checked_add(scratch.len()).ok_or(E::Bounds)?;
    if held > budget
        || scratch.len()
            < entries
                .iter()
                .map(|e| e.template.len())
                .max()
                .ok_or(E::Bounds)?
    {
        return Err(E::Bounds);
    }
    let mut destination = Caller {
        bytes: output,
        result: ContinuationCandidateRanges {
            ranges: [[0; 2]; crate::MAX_BODY_PLOTS],
            count: 0,
            used: 0,
        },
    };
    build_destination::<PORTS, _, _>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        resources,
        &mut destination,
        scratch,
        held,
        budget,
    )?;
    Ok(destination.result)
}
