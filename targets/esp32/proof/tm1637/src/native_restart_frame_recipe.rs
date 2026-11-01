//! Bounded encoded changed pieces; source ranges retain no copied source bytes.
//! This representation grants neither source authentication nor flash authority.
use super::*;
#[derive(Clone, Copy)]
pub(crate) enum ChangedPart {
    Prefix,
    Append,
    Suffix,
}
pub(crate) enum EncodedPiece<'a> {
    Changed {
        part: ChangedPart,
        bytes: &'a [u8],
    },
    Reused {
        source_offset: usize,
        bytes: &'a [u8],
    },
}
impl EncodedPiece<'_> {
    pub(crate) fn bytes(&self) -> &[u8] {
        match self {
            Self::Changed { bytes, .. } | Self::Reused { bytes, .. } => bytes,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct Limits {
    pub prefix: usize,
    pub append: usize,
    pub suffix: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CaptureRefusal {
    Capacity,
    SourceOrder,
}
pub(crate) struct Recipe {
    prefix: Vec<u8>,
    append: Vec<u8>,
    suffix: Vec<u8>,
    source: Option<archive::Span>,
    limits: Limits,
    encoded: usize,
    order: u8,
}
impl Recipe {
    /// Reserve every changed-byte budget before invoking the actual encoder.
    /// Returned encoded ranges are structural evidence only. The caller must
    /// bind them to its actual authenticated SourceIdentity and retained owner.
    pub(crate) fn capture<T>(
        limits: Limits,
        encode: impl FnOnce(
            &mut dyn FnMut(EncodedPiece<'_>) -> Result<(), CaptureRefusal>,
        ) -> Result<T, StreamRefusal<CaptureRefusal>>,
    ) -> Result<(Self, T), StreamRefusal<CaptureRefusal>> {
        let total = limits
            .prefix
            .checked_add(limits.append)
            .and_then(|n| n.checked_add(limits.suffix));
        if total.is_none_or(|n| n > MAX_BYTES) {
            return Err(StreamRefusal::Sink(CaptureRefusal::Capacity));
        }
        let reserve = |n| {
            let mut v = Vec::new();
            v.try_reserve_exact(n)
                .map_err(|_| StreamRefusal::Sink(CaptureRefusal::Capacity))?;
            Ok(v)
        };
        let mut recipe = Self {
            prefix: reserve(limits.prefix)?,
            append: reserve(limits.append)?,
            suffix: reserve(limits.suffix)?,
            source: None,
            limits,
            encoded: 0,
            order: 0,
        };
        let result = encode(&mut |piece| recipe.accept(piece))?;
        Ok((recipe, result))
    }
    fn accept(&mut self, piece: EncodedPiece<'_>) -> Result<(), CaptureRefusal> {
        let (order, bytes) = match &piece {
            EncodedPiece::Changed { part, bytes } => (
                match part {
                    ChangedPart::Prefix => 0,
                    ChangedPart::Append => 2,
                    ChangedPart::Suffix => 3,
                },
                bytes.len(),
            ),
            EncodedPiece::Reused { bytes, .. } => (1, bytes.len()),
        };
        if order < self.order {
            return Err(CaptureRefusal::SourceOrder);
        }
        let encoded = self
            .encoded
            .checked_add(bytes)
            .filter(|n| *n <= MAX_BYTES)
            .ok_or(CaptureRefusal::Capacity)?;
        self.order = order;
        self.encoded = encoded;
        match piece {
            EncodedPiece::Reused {
                source_offset,
                bytes,
            } => {
                let end = source_offset
                    .checked_add(bytes.len())
                    .filter(|n| *n <= MAX_BYTES)
                    .ok_or(CaptureRefusal::Capacity)?;
                match &mut self.source {
                    Some(span) if span.end == source_offset => span.end = end,
                    None => {
                        self.source = Some(archive::Span {
                            start: source_offset,
                            end,
                        })
                    }
                    _ => return Err(CaptureRefusal::SourceOrder),
                }
            }
            EncodedPiece::Changed { part, bytes } => {
                let (out, maximum) = match part {
                    ChangedPart::Prefix => (&mut self.prefix, self.limits.prefix),
                    ChangedPart::Append => (&mut self.append, self.limits.append),
                    ChangedPart::Suffix => (&mut self.suffix, self.limits.suffix),
                };
                if out
                    .len()
                    .checked_add(bytes.len())
                    .is_none_or(|n| n > maximum)
                {
                    return Err(CaptureRefusal::Capacity);
                }
                // All permitted bytes were reserved before encoding started.
                out.extend_from_slice(bytes);
            }
        }
        Ok(())
    }
    pub(crate) fn changed_lengths(&self) -> [usize; 3] {
        [self.prefix.len(), self.append.len(), self.suffix.len()]
    }
    pub(crate) fn retained_capacities(&self) -> [usize; 3] {
        [
            self.prefix.capacity(),
            self.append.capacity(),
            self.suffix.capacity(),
        ]
    }
    pub(crate) fn source_range(&self) -> Option<archive::Span> {
        self.source
    }
    pub(crate) fn changed_parts(&self) -> [&[u8]; 3] {
        [&self.prefix, &self.append, &self.suffix]
    }
}
#[cfg(all(test, feature = "observation-store"))]
#[path = "native_restart_frame_recipe_tests.rs"]
mod tests;
