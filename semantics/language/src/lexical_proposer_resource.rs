//! Finite canonical dictionary shards for an additive lexical proposer.
//! This resource supplies alternatives; it does not admit a parse or fact.
use crate::{LanguageLexicalEntry, LanguageLexicalProfile};
use alloc::{boxed::Box, vec::Vec};
use conduit_core::{semantic_digest, validate_canonical_structured_value};
use conduit_plot::rust_binding::{
    PreparedNativeFamily, PreparedNativeFamilyLimits, PreparedNativeRustBinding,
};
use core::mem::size_of;

pub const MAXIMUM_LEXICAL_DICTIONARY_SHARDS: usize = 128;
pub const MAXIMUM_LEXICAL_DICTIONARY_ENTRIES: usize = 8192;

#[derive(Clone, Copy, Debug)]
pub struct LexicalDictionaryLimits {
    pub maximum_shards: usize,
    pub maximum_entries: usize,
    pub maximum_frame_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_preparation_peak_bytes: usize,
    pub native: PreparedNativeFamilyLimits,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LexicalDictionaryRefusal {
    Capacity,
    Frame,
    Native,
    Language,
    DuplicateSurface,
}
#[derive(Clone, Copy, Debug)]
pub struct LexicalDictionaryStorageReceipt {
    pub shards: usize,
    pub entries: usize,
    pub retained_heap_bytes_bound: usize,
    pub preparation_peak_heap_bytes_bound: usize,
    pub lookup_requested_bytes_bound: usize,
}
struct Shard {
    bytes: Box<[u8]>,
    identity: [u8; 32],
}
struct Entry {
    surface: Box<str>,
    shard: u16,
    ordinal: u16,
}
pub struct PreparedLexicalDictionary {
    shards: Box<[Shard]>,
    entries: Vec<Entry>,
    identity: [u8; 32],
    language: Box<str>,
    family: PreparedNativeFamily,
    receipt: LexicalDictionaryStorageReceipt,
    maximum_frame_bytes: usize,
}
/// Exact reviewed resource custody accompanies the supplied alternatives.
/// The caller must preserve this origin through subsequent qualified analysis.
pub struct ReviewedLexicalLookup {
    entry: LanguageLexicalEntry,
    canonical_entry: Vec<u8>,
    dictionary_identity: [u8; 32],
    shard_identity: [u8; 32],
    shard: u16,
    ordinal: u16,
}
impl ReviewedLexicalLookup {
    pub fn entry(&self) -> &LanguageLexicalEntry {
        &self.entry
    }
    pub fn canonical_entry(&self) -> &[u8] {
        &self.canonical_entry
    }
    pub const fn dictionary_identity(&self) -> [u8; 32] {
        self.dictionary_identity
    }
    pub const fn shard_identity(&self) -> [u8; 32] {
        self.shard_identity
    }
    pub const fn shard(&self) -> u16 {
        self.shard
    }
    pub const fn ordinal(&self) -> u16 {
        self.ordinal
    }
}
fn add(a: usize, b: usize) -> Result<usize, LexicalDictionaryRefusal> {
    a.checked_add(b).ok_or(LexicalDictionaryRefusal::Capacity)
}
fn mul(a: usize, b: usize) -> Result<usize, LexicalDictionaryRefusal> {
    a.checked_mul(b).ok_or(LexicalDictionaryRefusal::Capacity)
}
fn admit(n: usize, ceiling: usize) -> Result<(), LexicalDictionaryRefusal> {
    if n > ceiling {
        Err(LexicalDictionaryRefusal::Capacity)
    } else {
        Ok(())
    }
}
impl PreparedLexicalDictionary {
    pub fn prepare(
        input: Vec<Vec<u8>>,
        language: &str,
        limits: LexicalDictionaryLimits,
    ) -> Result<Self, LexicalDictionaryRefusal> {
        use LexicalDictionaryRefusal as R;
        if input.is_empty()
            || limits.maximum_shards > MAXIMUM_LEXICAL_DICTIONARY_SHARDS
            || limits.maximum_frame_bytes > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
            || limits.maximum_entries > MAXIMUM_LEXICAL_DICTIONARY_ENTRIES
            || input.len() > limits.maximum_shards
            || language.is_empty()
        {
            return Err(R::Capacity);
        }
        let mut input_storage = mul(input.capacity(), size_of::<Vec<u8>>())?;
        let mut frame_storage = 0;
        for frame in &input {
            admit(frame.len(), limits.maximum_frame_bytes)?;
            input_storage = add(input_storage, frame.capacity())?;
            frame_storage = add(frame_storage, frame.len())?;
        }
        // All newly retained allocations, including worst-case surface boxes and
        // actual preallocated index capacity. Sharing is conservatively ignored.
        let dictionary_bound = add(
            add(
                add(add(size_of::<Self>(), language.len())?, frame_storage)?,
                mul(input.len(), size_of::<Shard>())?,
            )?,
            add(
                mul(limits.maximum_entries, size_of::<Entry>())?,
                mul(limits.maximum_entries, 256)?,
            )?,
        )?;
        let digest_framing = add(frame_storage, mul(input.len(), 4)?)?;
        let other_peak = add(add(input_storage, dictionary_bound)?, digest_framing)?;
        admit(other_peak, limits.maximum_preparation_peak_bytes)?;
        admit(dictionary_bound, limits.maximum_retained_bytes)?;
        let mut native_limits = limits.native;
        native_limits.maximum_retained_bytes = native_limits
            .maximum_retained_bytes
            .min(limits.maximum_retained_bytes - dictionary_bound);
        native_limits.maximum_preparation_peak_bytes = native_limits
            .maximum_preparation_peak_bytes
            .min(limits.maximum_preparation_peak_bytes - other_peak);
        let mut family = PreparedNativeFamily::prepare(
            &[LanguageLexicalProfile::PREPARED_DESCRIPTOR],
            native_limits,
        )
        .map_err(|_| R::Native)?;
        let native = family.storage_receipt();
        let peak = add(
            other_peak,
            native.preparation_peak_heap_bytes_bound.max(add(
                native.retained_heap_bytes_bound,
                native.conversion_requested_bytes_bound,
            )?),
        )?;
        admit(peak, limits.maximum_preparation_peak_bytes)?;
        let retained = add(dictionary_bound, native.retained_heap_bytes_bound)?;
        admit(retained, limits.maximum_retained_bytes)?;
        let mut entries = Vec::with_capacity(limits.maximum_entries);
        let mut shards = Vec::with_capacity(input.len());
        let mut framing = Vec::with_capacity(digest_framing);
        for (shard_index, frame) in input.into_iter().enumerate() {
            // Complete original Native profile/child contracts are admitted;
            // no selected-entry shortcut can skip another entry's constraints.
            let profile: LanguageLexicalProfile = family.decode(&frame).map_err(|_| R::Native)?;
            if profile.language().get() != language {
                return Err(R::Language);
            }
            for (ordinal, entry) in profile.entries().iter().enumerate() {
                if entries.len() == limits.maximum_entries {
                    return Err(R::Capacity);
                }
                entries.push(Entry {
                    surface: entry.surface().as_str().into(),
                    shard: shard_index as u16,
                    ordinal: ordinal as u16,
                });
            }
            framing.extend_from_slice(
                &u32::try_from(frame.len())
                    .map_err(|_| R::Capacity)?
                    .to_le_bytes(),
            );
            framing.extend_from_slice(&frame);
            let identity = semantic_digest("language/lexical-dictionary-shard@1", &frame);
            shards.push(Shard {
                bytes: frame.into_boxed_slice(),
                identity,
            });
        }
        entries.sort_unstable_by(|a, b| a.surface.cmp(&b.surface));
        if entries
            .windows(2)
            .any(|pair| pair[0].surface == pair[1].surface)
        {
            return Err(R::DuplicateSurface);
        }
        let identity = semantic_digest("language/lexical-dictionary@1", &framing);
        let lookup = add(
            limits.maximum_frame_bytes,
            native.conversion_requested_bytes_bound,
        )?;
        admit(
            add(retained, lookup)?,
            limits.maximum_preparation_peak_bytes,
        )?;
        let receipt = LexicalDictionaryStorageReceipt {
            shards: shards.len(),
            entries: entries.len(),
            retained_heap_bytes_bound: retained,
            preparation_peak_heap_bytes_bound: peak.max(add(retained, lookup)?),
            lookup_requested_bytes_bound: lookup,
        };
        Ok(Self {
            shards: shards.into_boxed_slice(),
            entries,
            identity,
            language: language.into(),
            family,
            receipt,
            maximum_frame_bytes: limits.maximum_frame_bytes,
        })
    }
    pub fn language(&self) -> &str {
        &self.language
    }
    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }
    pub const fn storage_receipt(&self) -> LexicalDictionaryStorageReceipt {
        self.receipt
    }
    pub fn lookup(
        &mut self,
        surface: &str,
    ) -> Result<Option<ReviewedLexicalLookup>, LexicalDictionaryRefusal> {
        use LexicalDictionaryRefusal as R;
        let Ok(index) = self
            .entries
            .binary_search_by(|entry| entry.surface.as_ref().cmp(surface))
        else {
            return Ok(None);
        };
        let entry = &self.entries[index];
        let shard = &self.shards[usize::from(entry.shard)];
        let view = validate_canonical_structured_value(&shard.bytes).map_err(|_| R::Frame)?;
        let value = view
            .record_field("entries")
            .map_err(|_| R::Frame)?
            .ok_or(R::Frame)?
            .collection_index(entry.ordinal)
            .map_err(|_| R::Frame)?
            .ok_or(R::Frame)?;
        if value.type_bytes() != LanguageLexicalEntry::PREPARED_DESCRIPTOR.type_bytes {
            return Err(R::Frame);
        }
        let length = add(value.type_bytes().len(), value.value_node().len())?;
        admit(length, self.maximum_frame_bytes)?;
        let mut frame = Vec::with_capacity(length);
        frame.extend_from_slice(value.type_bytes());
        frame.extend_from_slice(value.value_node());
        let native = self.family.decode(&frame).map_err(|_| R::Native)?;
        Ok(Some(ReviewedLexicalLookup {
            entry: native,
            canonical_entry: frame,
            dictionary_identity: self.identity,
            shard_identity: shard.identity,
            shard: entry.shard,
            ordinal: entry.ordinal,
        }))
    }
}
