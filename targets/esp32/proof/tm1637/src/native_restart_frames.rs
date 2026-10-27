//! Independently compressed immutable archive frames; framing grants no authority.
use super::{archive::Span, compress, MAX_BYTES, MAX_PLAIN_BYTES};
pub(super) const MAGIC: &[u8; 8] = b"FRMBDY01";
pub(super) const PREFIX: u8 = 0;
pub(super) const ARCHIVE: u8 = 1;
pub(super) const SUFFIX: u8 = 2;
pub(super) const END: u8 = 3;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Range {
    pub encoded: Span,
    pub plain: Span,
}
pub(super) struct Parsed(Range);
pub(super) struct Validated(Range);
impl Parsed {
    pub(super) fn validate(self, span: Span) -> Result<Validated, &'static str> {
        if self.0.plain != span {
            return Err("restart-frame-archive-span");
        }
        Ok(Validated(self.0))
    }
}
impl Validated {
    pub(super) fn into_range(self) -> Range {
        self.0
    }
}
struct Chunks<R> {
    read: R,
    input: [u8; 128],
    at: usize,
    len: usize,
    raw: usize,
    remaining: usize,
    ended: bool,
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> Chunks<R> {
    fn byte(&mut self) -> Result<Option<u8>, ()> {
        if self.at == self.len {
            self.input.fill(0);
            self.len = (self.read)(&mut self.input)?;
            self.at = 0;
            if self.len > 128 {
                return Err(());
            }
            if self.len == 0 {
                return Ok(None);
            }
        }
        let b = self.input[self.at];
        self.at += 1;
        self.raw = self.raw.checked_add(1).ok_or(())?;
        if self.raw > MAX_BYTES {
            return Err(());
        }
        Ok(Some(b))
    }
    fn required(&mut self) -> Result<u8, ()> {
        self.byte()?.ok_or(())
    }
}
impl<R> Drop for Chunks<R> {
    fn drop(&mut self) {
        self.input.fill(0);
    }
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> compress::Input for Chunks<R> {
    fn input(&mut self, out: &mut [u8; 128]) -> Result<usize, ()> {
        if self.ended {
            return Ok(0);
        }
        if self.remaining == 0 {
            self.remaining = usize::from(self.required()?);
            if self.remaining > 128 {
                return Err(());
            }
            if self.remaining == 0 {
                self.ended = true;
                return Ok(0);
            }
        }
        let n = self.remaining.min(128);
        for b in &mut out[..n] {
            *b = self.required()?;
        }
        self.remaining -= n;
        Ok(n)
    }
}
pub(super) struct Decoder<R> {
    decoder: compress::Decoder<Chunks<R>>,
    kind: u8,
    plain: usize,
    archive_start: usize,
    archive_end: usize,
    plain_start: usize,
    plain_end: usize,
    done: bool,
}
impl<R: FnMut(&mut [u8; 128]) -> Result<usize, ()>> Decoder<R> {
    #[inline(never)]
    pub(super) fn with<T>(read: R, consume: impl FnOnce(&mut Self) -> T) -> Result<T, ()> {
        let mut chunks = Chunks {
            read,
            input: [0; 128],
            at: 0,
            len: 0,
            raw: 0,
            remaining: 0,
            ended: false,
        };
        for b in MAGIC {
            if chunks.required()? != *b {
                return Err(());
            }
        }
        if chunks.required()? != PREFIX {
            return Err(());
        }
        let mut decoder = Self {
            decoder: compress::Decoder::new(chunks, MAX_PLAIN_BYTES, MAX_BYTES),
            kind: PREFIX,
            plain: 0,
            archive_start: 0,
            archive_end: 0,
            plain_start: 0,
            plain_end: 0,
            done: false,
        };
        Ok(consume(&mut decoder))
    }
    pub(super) fn read(&mut self, out: &mut [u8; 128]) -> Result<usize, ()> {
        loop {
            if self.done {
                return Ok(0);
            }
            let n = self.decoder.read(out).map_err(|_| ())?;
            self.plain = self.plain.checked_add(n).ok_or(())?;
            if self.plain > MAX_PLAIN_BYTES {
                return Err(());
            }
            if n != 0 {
                return Ok(n);
            }
            let source = self.decoder.source_mut();
            if !source.ended || source.remaining != 0 {
                return Err(());
            }
            let boundary = source.raw;
            let next = source.required()?;
            match (self.kind, next) {
                (PREFIX, ARCHIVE | SUFFIX) => {
                    self.archive_start = boundary;
                    self.plain_start = self.plain;
                }
                (ARCHIVE, ARCHIVE | SUFFIX) => {}
                (SUFFIX, END) => {
                    if source.byte()?.is_some() {
                        return Err(());
                    }
                    self.done = true;
                    return Ok(0);
                }
                _ => return Err(()),
            }
            if next == SUFFIX {
                self.archive_end = boundary;
                self.plain_end = self.plain;
            }
            self.kind = next;
            source.ended = false;
            self.decoder.reset_frame(MAX_PLAIN_BYTES - self.plain);
        }
    }
    pub(super) fn finish(&self) -> Result<Parsed, ()> {
        if !self.done {
            return Err(());
        }
        Ok(Parsed(Range {
            encoded: Span {
                start: self.archive_start,
                end: self.archive_end,
            },
            plain: Span {
                start: self.plain_start,
                end: self.plain_end,
            },
        }))
    }
}
struct ChunkWriter<W> {
    write: W,
    buffer: [u8; 128],
    used: usize,
}
impl<W: FnMut(&[u8]) -> Result<(), ()>> ChunkWriter<W> {
    fn flush(&mut self) -> Result<(), ()> {
        if self.used != 0 {
            (self.write)(&[self.used as u8])?;
            (self.write)(&self.buffer[..self.used])?;
            self.buffer.fill(0);
            self.used = 0;
        }
        Ok(())
    }
}
impl<W> Drop for ChunkWriter<W> {
    fn drop(&mut self) {
        self.buffer.fill(0);
    }
}
impl<W: FnMut(&[u8]) -> Result<(), ()>> compress::Output for ChunkWriter<W> {
    fn output(&mut self, mut bytes: &[u8]) -> Result<(), ()> {
        while !bytes.is_empty() {
            let n = (128 - self.used).min(bytes.len());
            self.buffer[self.used..self.used + n].copy_from_slice(&bytes[..n]);
            self.used += n;
            bytes = &bytes[n..];
            if self.used == 128 {
                self.flush()?;
            }
        }
        Ok(())
    }
}
pub(super) struct Encoder<W> {
    encoder: compress::Encoder<ChunkWriter<W>>,
}
impl<W: FnMut(&[u8]) -> Result<(), ()>> Encoder<W> {
    pub(super) fn new(write: W) -> Self {
        Self {
            encoder: compress::Encoder::new(
                ChunkWriter {
                    write,
                    buffer: [0; 128],
                    used: 0,
                },
                MAX_PLAIN_BYTES,
                MAX_BYTES,
            ),
        }
    }
    pub(super) fn raw(&mut self, bytes: &[u8]) -> Result<(), ()> {
        let writer = self.encoder.output_mut();
        if writer.used != 0 {
            return Err(());
        }
        (writer.write)(bytes)
    }
    pub(super) fn frame(
        &mut self,
        tag: u8,
        produce: impl FnOnce(&mut dyn FnMut(&[u8]) -> Result<(), ()>) -> Result<(), ()>,
    ) -> Result<compress::Lengths, compress::Error> {
        self.encoder.reset_frame();
        self.raw(&[tag]).map_err(|_| compress::Error::Invalid)?;
        let mut refusal = compress::Error::Invalid;
        produce(&mut |bytes| {
            self.encoder.write(bytes).map_err(|e| {
                refusal = e;
            })
        })
        .map_err(|_| refusal)?;
        let lengths = self.encoder.finish()?;
        self.encoder
            .output_mut()
            .flush()
            .map_err(|_| compress::Error::Invalid)?;
        self.raw(&[0]).map_err(|_| compress::Error::Invalid)?;
        Ok(lengths)
    }
}
#[cfg(test)]
pub(super) fn encode<E>(
    tag: u8,
    produce: impl FnOnce(&mut dyn FnMut(&[u8]) -> Result<(), ()>) -> Result<(), ()>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), E>,
) -> Result<compress::Lengths, super::StreamRefusal<E>> {
    let mut error = None;
    let result = {
        let mut encoder = Encoder::new(|bytes: &[u8]| {
            sink(bytes).map_err(|e| {
                error = Some(e);
            })
        });
        encoder.frame(tag, produce)
    };
    if let Some(e) = error {
        return Err(super::StreamRefusal::Sink(e));
    }
    result.map_err(|e| {
        if e == compress::Error::Capacity {
            super::StreamRefusal::Capacity
        } else {
            super::StreamRefusal::Invalid
        }
    })
}
