//! Immutable encoded preparation for the affine owned checkpoint only.
use super::super::framed_rewrite::{
    self,
    recipe::{CaptureRefusal, ChangedPart, EncodedPiece, Limits, Recipe},
};
use super::*;
use crate::native_body_checkpoint::{
    incremental::{frozen::SourceBlocks, IncrementalRewrite},
    Checkpoint, Refusal, SourceIdentity, SourceSnapshot, Storage,
};
pub(crate) struct Measurement {
    source: SourceIdentity,
    dimensions: [usize; 3],
    length: usize,
    digest: [u8; 32],
    metadata: (archive::Summary, archive::Span),
    frames: frames::Range,
    reuse: archive::Span,
}
pub(crate) struct FrozenRewrite {
    recipe: Recipe,
    source: SourceBlocks,
}
impl FrozenRewrite {
    pub(crate) fn stage<S: Storage>(
        &self,
        writer: &mut IncrementalRewrite,
        journal: &mut Checkpoint<S>,
    ) -> Result<(), Refusal> {
        let parts = self.recipe.changed_parts();
        writer.prepare_frozen_chunk(journal, &self.source, parts[0], parts[1], parts[2])
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.recipe.retained_capacities().iter().sum::<usize>() + self.source.retained_hash_bytes()
    }
}
impl Measurement {
    #[cfg(test)]
    pub(crate) fn prefix_bytes(&self) -> usize {
        self.dimensions[0]
    }
    pub(crate) fn metadata(&self) -> (archive::Summary, archive::Span) {
        self.metadata.clone()
    }
}
impl PreparedRewrite<'_> {
    fn pieces<S: Storage>(
        &self,
        journal: &mut Checkpoint<S>,
        source: &SourceSnapshot,
        sink: &mut dyn FnMut(EncodedPiece<'_>) -> Result<(), CaptureRefusal>,
    ) -> Result<(archive::Summary, archive::Span, frames::Range), &'static str> {
        #[cfg(test)]
        ENCODING_PASSES.with(|n| n.set(n.get() + 1));
        journal
            .read_snapshot(source, |reader| {
                framed_rewrite::emit_pieces(
                    self.evidence,
                    self.pending,
                    self.admissions,
                    self.span,
                    self.prefix,
                    self.local_count,
                    self.layouts,
                    self.network,
                    self.summary.clone(),
                    self.reuse,
                    |out| reader.read(out).map_err(|_| ()),
                    sink,
                )
                .map_err(|e| match e {
                    StreamRefusal::Sink(CaptureRefusal::Capacity) => Refusal::Capacity,
                    _ => Refusal::Corrupt,
                })
            })
            .map_err(|e| match e {
                Refusal::Capacity => "checkpoint-frozen-capacity",
                _ => "checkpoint-frozen-source",
            })
    }
    /// A source with no authenticated framed reuse retains the existing route.
    pub(crate) fn measure_frozen<S: Storage>(
        &self,
        journal: &mut Checkpoint<S>,
        source: &SourceSnapshot,
    ) -> Result<Option<Measurement>, &'static str> {
        let Some(reuse) = self.reuse.filter(|_| self.framed) else {
            return Ok(None);
        };
        let mut dimensions = [0usize; 3];
        let mut length = 0usize;
        let mut hash = Sha256::new();
        let metadata = self.pieces(journal, source, &mut |piece| {
            if let EncodedPiece::Changed { part, bytes } = &piece {
                let index = match part {
                    ChangedPart::Prefix => 0,
                    ChangedPart::Append => 1,
                    ChangedPart::Suffix => 2,
                };
                dimensions[index] = dimensions[index]
                    .checked_add(bytes.len())
                    .ok_or(CaptureRefusal::Capacity)?;
            }
            length = length
                .checked_add(piece.bytes().len())
                .filter(|n| *n <= MAX_BYTES)
                .ok_or(CaptureRefusal::Capacity)?;
            hash.update(piece.bytes());
            Ok(())
        })?;
        Ok(Some(Measurement {
            source: source.identity(),
            dimensions,
            length,
            digest: hash.finalize().into(),
            metadata: (metadata.0, metadata.1),
            frames: metadata.2,
            reuse: reuse.encoded,
        }))
    }
    /// Capacity alone may fall back before effects; changed input is corruption.
    pub(crate) fn capture_frozen<S: Storage>(
        &self,
        journal: &mut Checkpoint<S>,
        source: &SourceSnapshot,
        binding: &[u8; 32],
        seed: &[u8; 32],
        measured: &Measurement,
    ) -> Result<Option<(FrozenRewrite, IncrementalRewrite)>, &'static str> {
        if source.identity() != measured.source || self.checked.source != measured.source {
            return Err("checkpoint-frozen-source");
        }
        let mut hash = Sha256::new();
        let mut length = 0usize;
        let mut invoked = false;
        let captured = Recipe::capture(
            Limits {
                prefix: measured.dimensions[0],
                append: measured.dimensions[1],
                suffix: measured.dimensions[2],
            },
            |sink| {
                invoked = true;
                self.pieces(journal, source, &mut |piece| {
                    hash.update(piece.bytes());
                    length += piece.bytes().len();
                    sink(piece)
                })
                .map_err(|e| {
                    if e == "checkpoint-frozen-capacity" {
                        StreamRefusal::Sink(CaptureRefusal::Capacity)
                    } else {
                        StreamRefusal::Invalid
                    }
                })
            },
        );
        let (recipe, metadata) = match captured {
            Ok(v) => v,
            Err(StreamRefusal::Sink(CaptureRefusal::Capacity)) if !invoked => return Ok(None),
            Err(_) => return Err("checkpoint-frozen-capture"),
        };
        if recipe.changed_lengths() != measured.dimensions
            || recipe.source_range() != Some(measured.reuse)
            || length != measured.length
            || <[u8; 32]>::from(hash.finalize()) != measured.digest
            || (metadata.0, metadata.1) != measured.metadata
            || metadata.2 != measured.frames
        {
            return Err("checkpoint-frozen-changed");
        }
        let index_bytes = if measured.reuse.start == measured.reuse.end {
            0
        } else {
            ((measured.reuse.end - 1) / 128 + 1 - measured.reuse.start / 128)
                .checked_mul(32)
                .ok_or("checkpoint-frozen-capacity")?
        };
        let blocks = match SourceBlocks::authenticate(
            journal,
            source,
            measured.reuse.start,
            measured.reuse.end,
            index_bytes,
        ) {
            Ok(v) => v,
            Err(Refusal::Capacity) => return Ok(None),
            Err(_) => return Err("checkpoint-frozen-source"),
        };
        let writer = IncrementalRewrite::prepare_frozen(
            journal,
            &blocks,
            binding,
            seed,
            measured.length,
            measured.digest,
        )
        .map_err(|_| "checkpoint-frozen-writer")?;
        Ok(Some((
            FrozenRewrite {
                recipe,
                source: blocks,
            },
            writer,
        )))
    }
}
