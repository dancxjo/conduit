//! Freeze bytes from an actual preselected file capability, never a pathname.
use super::*;
use crate::source_file::{CapturedSourceFile, SourceFileRefusal};
use conduit_plot::{
    canonical_source_document_id, CheckContextRevision, SourceCheckLimits, SourceSnapshotRevision,
    MAXIMUM_PLOT_SOURCE_BYTES, MAX_CHECK_LABEL_BYTES,
};
use sha2::{Digest, Sha256};
use std::fs::File;

/// Immutable input acquired from an already selected readable File. There is no
/// path entrance, deserializer or mutable byte access. Owner/generation/context
/// labels describe this capture and never authorize access. The caller owns
/// resource selection and revision policy. This value does not establish file
/// publication/atomicity, catalogue authority, Host grants, memory admission or
/// a Ready provider.
pub struct OwnedSourceCheckInput {
    input: SourceCheckProcessInput,
}
impl OwnedSourceCheckInput {
    /// Preparation only. Consumes the descriptor and freezes its UTF-8 bytes.
    /// Root separately admits this buffer, the selected file IO and compiler
    /// workspace before invoking these effects.
    pub fn from_file(
        file: File,
        maximum_bytes: usize,
        owner: &str,
        generation: u64,
        entry: &str,
        context: CheckContextRevision,
    ) -> Result<Self, SourceFileRefusal> {
        if maximum_bytes == 0 || maximum_bytes > MAXIMUM_PLOT_SOURCE_BYTES {
            return Err(SourceFileRefusal::Bounds);
        }
        if generation == 0
            || [
                owner,
                entry,
                &context.grammar_edition,
                &context.catalog,
                &context.kinds_fores,
                &context.types_laws,
                &context.backs,
            ]
            .iter()
            .any(|label| label.is_empty() || label.len() > MAX_CHECK_LABEL_BYTES)
        {
            return Err(SourceFileRefusal::Scope);
        }
        let captured = CapturedSourceFile::from_file(file, maximum_bytes)?;
        let source = captured.into_source();
        let snapshot = SourceSnapshotRevision {
            owner: owner.into(),
            generation,
            sha256: format!("{:x}", Sha256::digest(source.as_bytes())),
            source_document_id: canonical_source_document_id(&source).0,
        };
        Ok(Self {
            input: SourceCheckProcessInput {
                source,
                owner: owner.into(),
                generation,
                request: SourceCheckRequest {
                    snapshot,
                    context,
                    entry: entry.into(),
                    limits: SourceCheckLimits::default(),
                },
            },
        })
    }
    /// Complete owned payload backing, including spare String capacities.
    /// This is an accounting observation, not a memory grant or Source authority.
    pub fn owned_backing_bytes(&self) -> Option<usize> {
        let request = &self.input.request;
        [
            &self.input.source,
            &self.input.owner,
            &request.snapshot.owner,
            &request.snapshot.sha256,
            &request.snapshot.source_document_id,
            &request.entry,
            &request.context.grammar_edition,
            &request.context.catalog,
            &request.context.kinds_fores,
            &request.context.types_laws,
            &request.context.backs,
        ]
        .into_iter()
        .try_fold(core::mem::size_of::<Self>(), |bytes, text| {
            bytes.checked_add(text.capacity())
        })
    }
    pub fn request(&self) -> &SourceCheckRequest {
        &self.input.request
    }
    /// Retain this capture's exact descriptive request without rereading the
    /// resource or copying its Source bytes. The caller independently admits
    /// retained metadata; the request grants no access or execution authority.
    pub fn into_request(self) -> SourceCheckRequest {
        self.input.request
    }
    /// Transfer the exact captured bytes into independently funded Root checking
    /// without reopening the file or cloning its Source. The descriptive request
    /// remains paired with that capture; neither value authorizes adoption.
    pub fn into_source_and_request(self) -> (String, SourceCheckRequest) {
        (self.input.source, self.input.request)
    }
    pub fn source(&self) -> &str {
        &self.input.source
    }
}
impl SourceCheckProcessWorker {
    /// Uses the retained capture. Labels cannot reopen a resource or replace its
    /// bytes; the existing worker still checks the actual selected executable
    /// and child context. A completed report confers no execution authority.
    pub fn prepare_selected_source(
        &mut self,
        source: &OwnedSourceCheckInput,
        control: &crate::RunControl,
    ) -> Result<SourceCheckProcessOutput, SourceCheckProcessFailure> {
        self.prepare(&source.input, control)
    }
}
#[cfg(test)]
#[path = "source/tests.rs"]
mod tests;
