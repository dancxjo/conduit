//! Borrowed inspection of an actual Session's retained evidence.
//! Views, ordinals and counters confer no fact, commitment or playback authority.
//! This module neither reconstructs missing evidence nor infers acknowledgments.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionEncoding {
    CanonicalNativeValue,
    CanonicalStructuredValue,
    CanonicalExpressionProgram,
    /// Exact generated UTF-8 hex lines, not decoded binary program bytes.
    ExactExpressionProgramHexLines,
    CanonicalType,
    ExactArtifactResource,
    ExactUtf8Manifest,
    ExactSourceUtf8,
}

/// Complete original material, never a digest, summary, Debug or retyped copy.
#[derive(Clone, Copy, Debug)]
pub struct InspectionMaterial<'a> {
    pub encoding: InspectionEncoding,
    pub bytes: &'a [u8],
}

#[derive(Clone, Copy)]
pub struct ProfileView<'a> {
    pub manifest: InspectionMaterial<'a>,
    pub proposer_definition: Option<InspectionMaterial<'a>>,
    pub proposal_dictionary: Option<InspectionMaterial<'a>>,
    pub model_signature: Option<InspectionMaterial<'a>>,
    pub artifact_resource: Option<InspectionMaterial<'a>>,
    #[cfg(feature = "parser-model-selection")]
    pub artifact_metadata: Option<&'a conduit_ai::ModelArtifact>,
    #[cfg(feature = "parser-model-selection")]
    pub signature: Option<&'a conduit_ai::ModelSignature>,
}

#[derive(Clone, Copy, Debug)]
pub struct RevisionView<'a> {
    pub revision: InspectionMaterial<'a>,
    pub proposed_tape: InspectionMaterial<'a>,
    pub lexical_tape: InspectionMaterial<'a>,
    pub source_lineage: InspectionMaterial<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontierKind {
    Lexical,
    Syntax,
    Prosody,
    Synthesis,
    Played,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtectionScope {
    /// Interpretation-epoch protection is not completed-token playback.
    InterpretationEpochOnly,
    ExactOccurrences,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontierCommitment {
    Unobserved,
    Proposed,
    Stable,
    Committed,
    Queued,
    Played,
}

#[derive(Clone, Copy, Debug)]
pub enum FrontierPosition<'a> {
    Unobserved,
    TokenPrefix {
        tokens: u64,
        basis: InspectionMaterial<'a>,
    },
    InterpretationEpoch {
        basis: InspectionMaterial<'a>,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct FrontierView<'a> {
    pub kind: FrontierKind,
    pub commitment: FrontierCommitment,
    pub position: FrontierPosition<'a>,
    pub protection_scope: Option<ProtectionScope>,
    /// Original full receipt/commitment when observed. A queued frame does not
    /// become played evidence through this view.
    pub evidence: Option<InspectionMaterial<'a>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExecutionEventKind {
    Source,
    Model,
    Admission,
    Refusal,
}

/// A retained observation from the original admission, never a new refusal
/// inferred from event ordinals, missing outputs or subsequent replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InspectionRefusal {
    NativeInvariant { original_law_index: usize },
}

/// Ordered original frames, including every retained intermediate. The driver
/// supplies exact rejected material for refusals, rather than inventing output.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionEventView<'a> {
    pub kind: ExecutionEventKind,
    /// Actual selected Source or model step; never inferred from an ordinal.
    pub step_name: &'a str,
    pub ordinal: u64,
    pub epoch: u64,
    pub model_call: Option<u64>,
    pub original_program: Option<InspectionMaterial<'a>>,
    pub source_custody: Option<InspectionMaterial<'a>>,
    /// Borrow the original checked Plans rather than labeling Debug or JSON as
    /// a portable Native encoding. Pure Source evaluation has no execution Plan.
    pub source_plan: Option<&'a conduit_core::Plan>,
    pub model_plan: Option<&'a conduit_core::Plan>,
    /// Complete canonical Types embedded in the corresponding retained frames.
    /// Malformed or absent frames have no embedded validated Type. Expected
    /// declared Types remain separately inspectable in the original Plans.
    pub input_type: Option<InspectionMaterial<'a>>,
    pub output_type: Option<InspectionMaterial<'a>>,
    pub input: InspectionMaterial<'a>,
    pub output: Option<InspectionMaterial<'a>>,
    pub intermediates: InspectionFrames<'a>,
    pub rejected_material: Option<InspectionMaterial<'a>>,
    pub refusal: Option<InspectionRefusal>,
}

/// Implemented on the original retained history owner; no self-referential
/// array of derived views is stored alongside its bytes.
pub(crate) trait InspectionFrameSource {
    fn frame_count(&self) -> usize;
    fn frame(&self, index: usize) -> Option<InspectionMaterial<'_>>;
}

#[derive(Clone, Copy)]
pub struct InspectionFrames<'a> {
    source: Option<&'a dyn InspectionFrameSource>,
}
impl<'a> InspectionFrames<'a> {
    pub(crate) fn new(source: &'a dyn InspectionFrameSource) -> Self {
        Self {
            source: Some(source),
        }
    }
    pub(crate) fn empty() -> Self {
        Self { source: None }
    }
    pub fn len(&self) -> usize {
        self.source.map_or(0, InspectionFrameSource::frame_count)
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn get(&self, index: usize) -> Option<InspectionMaterial<'a>> {
        self.source.and_then(|source| source.frame(index))
    }
    pub fn iter(&self) -> InspectionFrameIter<'a> {
        InspectionFrameIter {
            frames: *self,
            next: 0,
            end: self.len(),
        }
    }
}
impl core::fmt::Debug for InspectionFrames<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("InspectionFrames")
            .field("len", &self.len())
            .finish()
    }
}
pub struct InspectionFrameIter<'a> {
    frames: InspectionFrames<'a>,
    next: usize,
    end: usize,
}
impl<'a> Iterator for InspectionFrameIter<'a> {
    type Item = InspectionMaterial<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.end {
            return None;
        }
        let frame = self.frames.get(self.next);
        if frame.is_some() {
            self.next += 1;
        } else {
            self.next = self.end;
        }
        frame
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.end - self.next))
    }
}
impl core::iter::FusedIterator for InspectionFrameIter<'_> {}

#[derive(Clone, Copy, Debug)]
pub struct StorageView {
    pub retained_bytes_bound: usize,
    pub preparation_peak_bytes_bound: usize,
    pub retained_source_executions: usize,
    pub maximum_source_executions: usize,
    pub retained_model_executions: usize,
    pub maximum_model_executions: usize,
    pub retained_events: usize,
    pub maximum_events: usize,
}

/// Only the actual Session driver implements this seam. Returned material must
/// remain complete and immutable for the borrow, with stable ordered indices.
pub(crate) trait InspectionSource {
    fn profile(&self) -> ProfileView<'_>;
    fn revision(&self) -> Option<RevisionView<'_>>;
    fn frontiers(&self) -> [FrontierView<'_>; 5];
    fn event_count(&self) -> usize;
    fn event(&self, index: usize) -> Option<ExecutionEventView<'_>>;
    fn storage(&self) -> StorageView;
}

/// A read-only borrow supplied by the public Session, never caller-created
/// evidence that a Session was admitted or executed.
pub struct ParserSessionInspector<'a> {
    source: &'a dyn InspectionSource,
}

impl<'a> ParserSessionInspector<'a> {
    pub(crate) fn new(source: &'a dyn InspectionSource) -> Self {
        Self { source }
    }
    pub fn profile(&self) -> ProfileView<'_> {
        self.source.profile()
    }
    pub fn revision(&self) -> Option<RevisionView<'_>> {
        self.source.revision()
    }
    pub fn frontiers(&self) -> [FrontierView<'_>; 5] {
        self.source.frontiers()
    }
    pub fn storage(&self) -> StorageView {
        self.source.storage()
    }
    pub fn event_count(&self) -> usize {
        self.source.event_count()
    }
    pub fn event(&self, index: usize) -> Option<ExecutionEventView<'_>> {
        self.source.event(index)
    }
    pub fn events(&self) -> InspectionEvents<'_> {
        InspectionEvents {
            source: self.source,
            next: 0,
            end: self.source.event_count(),
        }
    }
}

/// Allocation-free traversal; indices locate evidence and grant no authority.
pub struct InspectionEvents<'a> {
    source: &'a dyn InspectionSource,
    next: usize,
    end: usize,
}
impl<'a> Iterator for InspectionEvents<'a> {
    type Item = ExecutionEventView<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.end {
            return None;
        }
        let event = self.source.event(self.next);
        if event.is_some() {
            self.next += 1;
        } else {
            self.next = self.end;
        }
        event
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.end - self.next))
    }
}
impl core::iter::FusedIterator for InspectionEvents<'_> {}
