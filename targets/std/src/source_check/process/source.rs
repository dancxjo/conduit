//! Freeze bytes from an actual preselected file capability, never a pathname.
use super::*;
use conduit_plot::{
    canonical_source_document_id, CheckContextRevision, SourceCheckLimits, SourceSnapshotRevision,
    MAXIMUM_PLOT_SOURCE_BYTES, MAX_CHECK_LABEL_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Seek, SeekFrom},
};

#[derive(Debug)]
pub enum SourceFileRefusal {
    Bounds,
    Scope,
    Metadata(std::io::Error),
    NotRegular,
    Bytes,
    ExtentChanged,
    Read(std::io::Error),
    Utf8,
}
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
        mut file: File,
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
        let metadata = file.metadata().map_err(SourceFileRefusal::Metadata)?;
        if !metadata.is_file() {
            return Err(SourceFileRefusal::NotRegular);
        }
        let extent = usize::try_from(metadata.len()).map_err(|_| SourceFileRefusal::Bytes)?;
        if extent > maximum_bytes {
            return Err(SourceFileRefusal::Bytes);
        }
        file.seek(SeekFrom::Start(0))
            .map_err(SourceFileRefusal::Read)?;
        let bytes = capture_extent(&mut file, extent)?;
        let source = String::from_utf8(bytes).map_err(|_| SourceFileRefusal::Utf8)?;
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
    pub fn request(&self) -> &SourceCheckRequest {
        &self.input.request
    }
    pub fn source(&self) -> &str {
        &self.input.source
    }
}
// One fixed extent plus an overflow sentinel. Contrary evidence never grows
// storage, silently changes the admitted extent, or starts an unbounded retry.
fn capture_extent(mut input: impl Read, extent: usize) -> Result<Vec<u8>, SourceFileRefusal> {
    if extent > MAXIMUM_PLOT_SOURCE_BYTES {
        return Err(SourceFileRefusal::Bounds);
    }
    let mut bytes = vec![0; extent + 1];
    let mut used = 0;
    while used < bytes.len() {
        match input.read(&mut bytes[used..]) {
            Ok(0) => break,
            Ok(count) => used += count,
            Err(error) => return Err(SourceFileRefusal::Read(error)),
        }
    }
    if used != extent {
        return Err(SourceFileRefusal::ExtentChanged);
    }
    bytes.truncate(used);
    Ok(bytes)
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
