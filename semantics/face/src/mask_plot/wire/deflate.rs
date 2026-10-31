//! Bounded deterministic RFC1950/1951 dynamic-Huffman stream. Only the selected
//! Only a 10KiB history and packed bounded hash links are retained. Dictionary
//! allocations are at most512 bytes; there is no whole-document or miniz workspace.
use super::{huffman, CodecError};
use alloc::{boxed::Box, string::String, vec};
use sha2::{Digest, Sha256};
const WINDOW: usize = 10240;
const LOOKAHEAD: usize = 258;
const HASHES: usize = 256;
const PROBES: usize = 128;
const NONE: u32 = u32::MAX;
const LENGTH_BASE: [usize; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [usize; 27] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193,
];
const DISTANCE_EXTRA: [u8; 27] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12,
];
pub(super) const WORKSPACE_BYTES: usize = WINDOW
    + (WINDOW * 3 / 2).div_ceil(CHUNK) * CHUNK
    + HASHES * 4
    + core::mem::size_of::<Encoder<'static>>()
    + 316 * 4;
type CountedStream = (Box<[u32]>, usize, [u8; 32]);
const CHUNK: usize = 512;
enum Block<'a> {
    Owned(Box<[u8]>),
    Borrowed(&'a mut [u8]),
}
impl Block<'_> {
    fn get(&self, index: usize) -> u8 {
        match self {
            Self::Owned(bytes) => bytes[index],
            Self::Borrowed(bytes) => bytes[index],
        }
    }
    fn set(&mut self, index: usize, value: u8) {
        match self {
            Self::Owned(bytes) => bytes[index] = value,
            Self::Borrowed(bytes) => bytes[index] = value,
        }
    }
}
struct Chunks<'a, const N: usize>([Block<'a>; N]);
impl<'a, const N: usize> Chunks<'a, N> {
    fn new(chunks: &mut impl Iterator<Item = &'a mut [u8]>) -> Self {
        Self(core::array::from_fn(|_| match chunks.next() {
            Some(bytes) => {
                bytes.fill(0);
                Block::Borrowed(bytes)
            }
            None => Block::Owned(vec![0; CHUNK].into_boxed_slice()),
        }))
    }
    fn get(&self, index: usize) -> u8 {
        self.0[index / CHUNK].get(index % CHUNK)
    }
    fn set(&mut self, index: usize, value: u8) {
        self.0[index / CHUNK].set(index % CHUNK, value);
    }
}
struct Heads([Box<[u32]>; HASHES / 128]);
impl Heads {
    fn new() -> Self {
        Self(core::array::from_fn(|_| vec![NONE; 128].into_boxed_slice()))
    }
    fn get(&self, index: usize) -> u32 {
        self.0[index / 128][index % 128]
    }
    fn set(&mut self, index: usize, value: u32) {
        self.0[index / 128][index % 128] = value;
    }
}
pub(super) struct Encoder<'a> {
    tree: Option<Box<huffman::Tree>>,
    histogram: Option<Box<[u32]>>,
    digest: Sha256,
    history: Chunks<'a, { WINDOW / CHUNK }>,
    previous: Chunks<'a, { (WINDOW * 3 / 2).div_ceil(CHUNK) }>,
    heads: Heads,
    pending: [u8; LOOKAHEAD],
    pending_len: usize,
    position: u32,
    expanded: usize,
    maximum_expanded: usize,
    maximum_compressed: usize,
    adler_a: u32,
    adler_b: u32,
    bits: u32,
    bit_count: u8,
    encoded: String,
    compressed: usize,
    transport_bits: u32,
    transport_len: u8,
}
impl<'a> Encoder<'a> {
    pub(super) fn new(
        maximum_expanded: usize,
        maximum_compressed: usize,
    ) -> Result<Self, CodecError> {
        Self::from_chunks(
            maximum_expanded,
            maximum_compressed,
            &mut core::iter::empty(),
        )
    }
    fn new_with_workspace(
        maximum_expanded: usize,
        maximum_compressed: usize,
        workspace: &'a mut [&mut [u8]],
    ) -> Result<Self, CodecError> {
        if workspace.is_empty() {
            return Self::new(maximum_expanded, maximum_compressed);
        }
        let mut chunks = workspace
            .iter_mut()
            .flat_map(|bytes| bytes.as_chunks_mut::<CHUNK>().0.iter_mut())
            .map(|chunk| chunk.as_mut_slice());
        Self::from_chunks(maximum_expanded, maximum_compressed, &mut chunks)
    }
    fn from_chunks(
        maximum_expanded: usize,
        maximum_compressed: usize,
        chunks: &mut impl Iterator<Item = &'a mut [u8]>,
    ) -> Result<Self, CodecError> {
        let mut encoder = Self {
            tree: None,
            histogram: None,
            digest: Sha256::new(),
            history: Chunks::new(chunks),
            previous: Chunks::new(chunks),
            heads: Heads::new(),
            pending: [0; LOOKAHEAD],
            pending_len: 0,
            position: 0,
            expanded: 0,
            maximum_expanded,
            maximum_compressed,
            adler_a: 1,
            adler_b: 0,
            bits: 0,
            bit_count: 0,
            encoded: String::with_capacity(
                512.min(maximum_compressed.div_ceil(3).saturating_mul(4)),
            ),
            compressed: 0,
            transport_bits: 0,
            transport_len: 0,
        };
        encoder.byte(0x78)?;
        encoder.byte(0x01)?;
        encoder.header()?;
        Ok(encoder)
    }
    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<(), CodecError> {
        if self
            .expanded
            .checked_add(bytes.len())
            .is_none_or(|len| len > self.maximum_expanded)
        {
            return Err(CodecError::Capacity);
        }
        self.expanded += bytes.len();
        self.digest.update(bytes);
        for &byte in bytes {
            self.adler_a = (self.adler_a + u32::from(byte)) % 65521;
            self.adler_b = (self.adler_b + self.adler_a) % 65521;
            self.pending[self.pending_len] = byte;
            self.pending_len += 1;
            if self.pending_len == LOOKAHEAD {
                self.token()?;
            }
        }
        Ok(())
    }
    fn complete(&mut self) -> Result<(), CodecError> {
        if self.expanded == 0 {
            return Err(CodecError::Invalid);
        }
        while self.pending_len != 0 {
            self.token()?;
        }
        self.symbol(256)?;
        if self.bit_count != 0 {
            self.byte(self.bits as u8)?;
            self.bits = 0;
            self.bit_count = 0;
        }
        let checksum = (self.adler_b << 16) | self.adler_a;
        for byte in checksum.to_be_bytes() {
            self.byte(byte)?;
        }
        const BASE64: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        if self.transport_len != 0 {
            self.reserve_quad()?;
            let bits = self.transport_bits << (8 * (3 - self.transport_len));
            self.encoded
                .push(BASE64[((bits >> 18) & 63) as usize] as char);
            self.encoded
                .push(BASE64[((bits >> 12) & 63) as usize] as char);
            self.encoded.push(if self.transport_len == 2 {
                BASE64[((bits >> 6) & 63) as usize] as char
            } else {
                '='
            });
            self.encoded.push('=');
            self.transport_len = 0;
        }
        Ok(())
    }
    pub(super) fn finish(mut self) -> Result<(String, usize, [u8; 32]), CodecError> {
        self.complete()?;
        Ok((self.encoded, self.expanded, self.digest.finalize().into()))
    }
    pub(super) fn counting(maximum_expanded: usize) -> Result<Self, CodecError> {
        let mut encoder = Self::new(maximum_expanded, 64 * 1024)?;
        encoder.encoded.clear();
        encoder.compressed = 0;
        encoder.transport_bits = 0;
        encoder.transport_len = 0;
        encoder.histogram = Some(vec![0; 316].into_boxed_slice());
        Ok(encoder)
    }
    pub(super) fn counting_with_workspace(
        maximum_expanded: usize,
        workspace: &'a mut [&mut [u8]],
    ) -> Result<Self, CodecError> {
        if workspace.is_empty() {
            return Self::counting(maximum_expanded);
        }
        let mut encoder = Self::new_with_workspace(maximum_expanded, 64 * 1024, workspace)?;
        encoder.encoded.clear();
        encoder.compressed = 0;
        encoder.transport_bits = 0;
        encoder.transport_len = 0;
        encoder.histogram = Some(vec![0; 316].into_boxed_slice());
        Ok(encoder)
    }
    pub(super) fn finish_count(mut self) -> Result<CountedStream, CodecError> {
        self.complete()?;
        Ok((
            self.histogram.take().ok_or(CodecError::Invalid)?,
            self.expanded,
            self.digest.finalize().into(),
        ))
    }
    pub(super) fn with_tree(
        maximum_expanded: usize,
        maximum_compressed: usize,
        tree: Box<huffman::Tree>,
    ) -> Result<Self, CodecError> {
        let mut encoder = Self::new(maximum_expanded, maximum_compressed)?;
        encoder.encoded.clear();
        encoder.compressed = 0;
        encoder.transport_bits = 0;
        encoder.transport_len = 0;
        encoder.bits = 0;
        encoder.bit_count = 0;
        encoder.tree = Some(tree);
        encoder.byte(0x78)?;
        encoder.byte(0x01)?;
        encoder.header()?;
        Ok(encoder)
    }
    pub(super) fn with_tree_and_workspace(
        maximum_expanded: usize,
        maximum_compressed: usize,
        tree: Box<huffman::Tree>,
        workspace: &'a mut [&mut [u8]],
    ) -> Result<Self, CodecError> {
        if workspace.is_empty() {
            return Self::with_tree(maximum_expanded, maximum_compressed, tree);
        }
        let mut encoder =
            Self::new_with_workspace(maximum_expanded, maximum_compressed, workspace)?;
        encoder.encoded.clear();
        encoder.compressed = 0;
        encoder.transport_bits = 0;
        encoder.transport_len = 0;
        encoder.bits = 0;
        encoder.bit_count = 0;
        encoder.tree = Some(tree);
        encoder.byte(0x78)?;
        encoder.byte(0x01)?;
        encoder.header()?;
        Ok(encoder)
    }
    fn hash(a: u8, b: u8, c: u8) -> usize {
        usize::from(a.rotate_left(3) ^ b.rotate_left(1) ^ c)
    }
    fn best_match(&self, offset: usize) -> (usize, usize, usize) {
        let (mut best_length, mut best_distance, mut best_saving) = (0, 0, 0);
        if self.pending_len < offset + 3 {
            return (0, 0, 0);
        }
        let position = self.position + offset as u32;
        let mut candidate = self.heads.get(Self::hash(
            self.pending[offset],
            self.pending[offset + 1],
            self.pending[offset + 2],
        ));
        for _ in 0..PROBES {
            if candidate == NONE || candidate >= position {
                break;
            }
            let distance = (position - candidate) as usize;
            if distance > WINDOW {
                break;
            }
            let mut length = 0;
            while length < self.pending_len - offset {
                let source = candidate as usize + length;
                let expected = if source >= self.position as usize {
                    self.pending[source - self.position as usize]
                } else {
                    self.history.get(source % WINDOW)
                };
                if expected != self.pending[offset + length] {
                    break;
                }
                length += 1;
            }
            if length >= 3 {
                // Both passes use immutable parse costs. Derived output codes
                // cannot alter token selection, so absent first-pass symbols
                // may have zero codes; any unexpected symbol fails closed.
                let length_index = LENGTH_BASE
                    .iter()
                    .rposition(|&base| base <= length)
                    .unwrap();
                let distance_index = DISTANCE_BASE
                    .iter()
                    .rposition(|&base| base <= distance)
                    .unwrap();
                let match_bits = usize::from(huffman::LENGTHS[257 + length_index])
                    + usize::from(LENGTH_EXTRA[length_index])
                    + if distance_index < 2 { 4 } else { 5 }
                    + usize::from(DISTANCE_EXTRA[distance_index]);
                let literal_bits: usize = self.pending[offset..offset + length]
                    .iter()
                    .map(|&byte| usize::from(huffman::LENGTHS[usize::from(byte)]))
                    .sum();
                let saving = literal_bits.saturating_sub(match_bits);
                if saving > best_saving {
                    best_length = length;
                    best_distance = distance;
                    best_saving = saving;
                }
            }
            let delta = self.previous_link(candidate as usize % WINDOW);
            if delta == 0 || candidate < u32::from(delta) {
                break;
            }
            candidate -= u32::from(delta);
        }
        (best_length, best_distance, best_saving)
    }
    fn token(&mut self) -> Result<(), CodecError> {
        let (mut best_length, best_distance, best_saving) = self.best_match(0);
        // One bounded lazy decision avoids spending a short prefix match that
        // hides a longer following match; no extra dictionary/storage is used.
        if best_length >= 3 && self.best_match(1).2 > best_saving {
            best_length = 0;
        }
        let consumed = if best_length >= 3 {
            let index = LENGTH_BASE
                .iter()
                .rposition(|&base| base <= best_length)
                .ok_or(CodecError::Invalid)?;
            self.symbol(257 + index as u16)?;
            self.write_bits(
                (best_length - LENGTH_BASE[index]) as u32,
                LENGTH_EXTRA[index],
            )?;
            let index = DISTANCE_BASE
                .iter()
                .rposition(|&base| base <= best_distance)
                .ok_or(CodecError::Invalid)?;
            if let Some(histogram) = &mut self.histogram {
                histogram[286 + index] += 1;
            }
            let bits = self.distance_length(index);
            if bits == 0 {
                return Err(CodecError::Invalid);
            }
            let code = self.tree.as_ref().map_or_else(
                || {
                    let code = if index < 2 { index } else { index + 2 };
                    (code as u16).reverse_bits() >> (16 - bits)
                },
                |tree| tree.distance_codes[index],
            );
            self.write_bits(u32::from(code), bits)?;
            self.write_bits(
                (best_distance - DISTANCE_BASE[index]) as u32,
                DISTANCE_EXTRA[index],
            )?;
            best_length
        } else {
            self.symbol(u16::from(self.pending[0]))?;
            1
        };
        for index in 0..consumed {
            self.remember(self.pending[index]);
        }
        self.pending.copy_within(consumed..self.pending_len, 0);
        self.pending_len -= consumed;
        Ok(())
    }
    fn previous_link(&self, index: usize) -> u16 {
        let slot = index * 3 / 2;
        if index.is_multiple_of(2) {
            u16::from(self.previous.get(slot)) | (u16::from(self.previous.get(slot + 1) & 15) << 8)
        } else {
            u16::from(self.previous.get(slot) >> 4) | (u16::from(self.previous.get(slot + 1)) << 4)
        }
    }
    fn set_previous_link(&mut self, index: usize, value: u16) {
        let slot = index * 3 / 2;
        if index.is_multiple_of(2) {
            self.previous.set(slot, value as u8);
            self.previous.set(
                slot + 1,
                (self.previous.get(slot + 1) & 240) | ((value >> 8) as u8),
            );
        } else {
            self.previous
                .set(slot, (self.previous.get(slot) & 15) | ((value as u8) << 4));
            self.previous.set(slot + 1, (value >> 4) as u8);
        }
    }
    fn remember(&mut self, byte: u8) {
        let position = self.position as usize;
        self.history.set(position % WINDOW, byte);
        if position >= 2 {
            let start = position - 2;
            let hash = Self::hash(
                self.history.get(start % WINDOW),
                self.history.get((start + 1) % WINDOW),
                byte,
            );
            let prior = self.heads.get(hash);
            let delta = if prior != NONE && start as u32 > prior && start as u32 - prior <= 4095 {
                (start as u32 - prior) as u16
            } else {
                0
            };
            self.set_previous_link(start % WINDOW, delta);
            self.heads.set(hash, start as u32);
        }
        self.position += 1;
    }
    fn header(&mut self) -> Result<(), CodecError> {
        self.write_bits(5, 3)?; // BFINAL=1, BTYPE=10 (dynamic tree).
        self.write_bits(29, 5)?; // 286 literal/length symbols.
        self.write_bits(29, 5)?; // All thirty permitted distance symbols.
        self.write_bits(15, 4)?; // All nineteen code-length alphabet entries.
        const ORDER: [usize; 19] = [
            16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
        ];
        let lengths = self
            .tree
            .as_ref()
            .map_or(huffman::LENGTHS, |tree| tree.lengths);
        let distances = core::array::from_fn::<_, 30, _>(|i| self.distance_length(i));
        let length = |index: usize| {
            if index < 286 {
                lengths[index]
            } else {
                distances[index - 286]
            }
        };
        let mut histogram = [0u32; 19];
        header_tokens(length, |symbol, _, _| {
            histogram[usize::from(symbol)] += 1;
            Ok(())
        })?;
        let mut code_lengths = core::array::from_fn::<_, 19, _>(|i| if i < 15 { 4 } else { 6 });
        let mut code_words = core::array::from_fn::<_, 19, _>(|i| {
            let (bits, code) = if i < 15 { (4, i) } else { (6, 60 + i - 15) };
            (code as u16).reverse_bits() >> (16 - bits)
        });
        huffman::derive(&histogram, &mut code_lengths, &mut code_words, 7);
        for symbol in ORDER {
            self.write_bits(u32::from(code_lengths[symbol]), 3)?;
        }
        header_tokens(length, |symbol, extra, bits| {
            let index = usize::from(symbol);
            self.write_bits(u32::from(code_words[index]), code_lengths[index])?;
            self.write_bits(extra, bits)
        })?;
        Ok(())
    }
    fn distance_length(&self, index: usize) -> u8 {
        self.tree
            .as_ref()
            .map_or(if index < 2 { 4 } else { 5 }, |tree| {
                tree.distance_lengths[index]
            })
    }
    fn length(&self, index: usize) -> u8 {
        self.tree
            .as_ref()
            .map_or(huffman::LENGTHS[index], |tree| tree.lengths[index])
    }
    fn symbol(&mut self, symbol: u16) -> Result<(), CodecError> {
        let index = usize::from(symbol);
        if index >= 286 {
            return Err(CodecError::Invalid);
        }
        if let Some(histogram) = &mut self.histogram {
            histogram[index] += 1;
        }
        let code = self
            .tree
            .as_ref()
            .map_or(huffman::CODES[index], |tree| tree.codes[index]);
        let length = self.length(index);
        if length == 0 {
            return Err(CodecError::Invalid);
        }
        self.write_bits(u32::from(code), length)
    }
    fn write_bits(&mut self, value: u32, count: u8) -> Result<(), CodecError> {
        self.bits |= value << self.bit_count;
        self.bit_count += count;
        while self.bit_count >= 8 {
            self.byte(self.bits as u8)?;
            self.bits >>= 8;
            self.bit_count -= 8;
        }
        Ok(())
    }
    fn reserve_quad(&mut self) -> Result<(), CodecError> {
        let needed = self
            .encoded
            .len()
            .checked_add(4)
            .ok_or(CodecError::Capacity)?;
        if needed > self.encoded.capacity() {
            let maximum = self
                .maximum_compressed
                .div_ceil(3)
                .checked_mul(4)
                .ok_or(CodecError::Capacity)?;
            if needed > maximum {
                return Err(CodecError::Capacity);
            }
            let capacity = self
                .encoded
                .capacity()
                .saturating_mul(2)
                .min(maximum)
                .max(needed);
            self.encoded.reserve_exact(capacity - self.encoded.len());
        }
        Ok(())
    }
    fn byte(&mut self, byte: u8) -> Result<(), CodecError> {
        if self.histogram.is_some() {
            return Ok(());
        }
        if self.compressed >= self.maximum_compressed {
            return Err(CodecError::Capacity);
        }
        self.compressed += 1;
        self.transport_bits = (self.transport_bits << 8) | u32::from(byte);
        self.transport_len += 1;
        if self.transport_len == 3 {
            self.reserve_quad()?;
            const BASE64: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            for shift in [18, 12, 6, 0] {
                self.encoded
                    .push(BASE64[((self.transport_bits >> shift) & 63) as usize] as char);
            }
            self.transport_bits = 0;
            self.transport_len = 0;
        }
        Ok(())
    }
}

fn header_tokens(
    length: impl Fn(usize) -> u8,
    mut emit: impl FnMut(u8, u32, u8) -> Result<(), CodecError>,
) -> Result<(), CodecError> {
    let mut index = 0;
    while index < 316 {
        let value = length(index);
        let mut count = 1;
        while index + count < 316 && length(index + count) == value {
            count += 1;
        }
        index += count;
        if value == 0 {
            while count >= 11 {
                let take = count.min(138);
                emit(18, (take - 11) as u32, 7)?;
                count -= take;
            }
            if count >= 3 {
                let take = count.min(10);
                emit(17, (take - 3) as u32, 3)?;
                count -= take;
            }
        } else {
            emit(value, 0, 0)?;
            count -= 1;
            while count >= 3 {
                let take = count.min(6);
                emit(16, (take - 3) as u32, 2)?;
                count -= take;
            }
        }
        for _ in 0..count {
            emit(value, 0, 0)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn non_power_of_two_output_limit_caps_capacity_before_partial_refusal() {
        let mut encoder = Encoder::new(262144, 1600).unwrap();
        let mut state = 0x1234_5678u32;
        let mut refused = false;
        for _ in 0..16000 {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            if encoder.write(&[(state >> 24) as u8]).is_err() {
                refused = true;
                break;
            }
        }
        assert!(refused);
        assert!(encoder.encoded.capacity() <= 1600usize.div_ceil(3) * 4);
    }
    #[test]
    fn packed_links_preserve_neighbors_across_every_chunk_and_ring_boundary() {
        let mut encoder = Encoder::new(262144, 65536).unwrap();
        for index in 0..WINDOW {
            encoder.set_previous_link(index, ((index * 73) & 4095) as u16);
        }
        for index in 0..WINDOW {
            assert_eq!(encoder.previous_link(index), ((index * 73) & 4095) as u16);
        }
        for index in (0..WINDOW).rev() {
            encoder.set_previous_link(index, ((index * 17 + 9) & 4095) as u16);
        }
        for index in 0..WINDOW {
            assert_eq!(
                encoder.previous_link(index),
                ((index * 17 + 9) & 4095) as u16
            );
        }
        assert!(core::mem::size_of::<Encoder<'static>>() < 2048);
        assert!(core::mem::size_of::<huffman::Tree>() < 1024);
    }
}
