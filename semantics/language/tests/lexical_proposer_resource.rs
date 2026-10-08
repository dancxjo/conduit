extern crate alloc;
use conduit_language::lexical_proposer_resource as resource;
use conduit_language::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding, PreparedNativeFamilyLimits};
fn native() -> PreparedNativeFamilyLimits {
    PreparedNativeFamilyLimits {
        maximum_types: 64,
        maximum_laws_per_type: 256,
        maximum_input_bytes: 262144,
        maximum_retained_bytes: 200_000_000,
        maximum_preparation_peak_bytes: 1_000_000_000,
        maximum_conversion_requested_bytes: 1_000_000_000,
    }
}
fn limits() -> resource::LexicalDictionaryLimits {
    resource::LexicalDictionaryLimits {
        maximum_shards: 128,
        maximum_entries: 8192,
        maximum_frame_bytes: 262144,
        maximum_retained_bytes: 300_000_000,
        maximum_preparation_peak_bytes: 2_000_000_000,
        native: native(),
    }
}
fn shard(word: &str, lang: &str) -> Vec<u8> {
    LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
                word.into(),
                BoundedSequence::new(),
                LanguageLexicalPos::Noun,
            )
            .unwrap()])
            .unwrap(),
            word.into(),
        )
        .unwrap()])
        .unwrap(),
        format!("shard/{word}"),
        LanguageId::new(lang.into()).unwrap(),
        LinguisticDerivationProvenance::deterministic_rule("reviewed-test".into(), "1".into())
            .unwrap(),
    )
    .unwrap()
    .encode()
    .unwrap()
}
#[test]
fn complete_shard_lookup_foreign_duplicate_and_quota_refusals() {
    let inputs = vec![shard("record", "language/en"), shard("old", "language/en")];
    let mut p =
        resource::PreparedLexicalDictionary::prepare(inputs.clone(), "language/en", limits())
            .unwrap();
    let r = p.storage_receipt();
    assert_eq!(r.shards, 2);
    assert_eq!(r.entries, 2);
    assert!(r.preparation_peak_heap_bytes_bound >= r.retained_heap_bytes_bound);
    assert!(r.lookup_requested_bytes_bound > 0);
    let lookup = p.lookup("old").unwrap().unwrap();
    assert_eq!(lookup.entry().surface(), "old");
    assert_eq!(
        LanguageLexicalEntry::decode(lookup.canonical_entry()).unwrap(),
        *lookup.entry()
    );
    assert_eq!(lookup.shard(), 1);
    assert_eq!(lookup.ordinal(), 0);
    assert_eq!(lookup.dictionary_identity(), p.identity());
    assert_ne!(lookup.dictionary_identity(), lookup.shard_identity());
    assert!(p.lookup("unlisted").unwrap().is_none());
    assert!(matches!(
        resource::PreparedLexicalDictionary::prepare(
            vec![inputs[0].clone(), inputs[0].clone()],
            "language/en",
            limits()
        ),
        Err(resource::LexicalDictionaryRefusal::DuplicateSurface)
    ));
    assert!(matches!(
        resource::PreparedLexicalDictionary::prepare(
            vec![shard("record", "language/fr")],
            "language/en",
            limits()
        ),
        Err(resource::LexicalDictionaryRefusal::Language)
    ));
    let mut tiny = limits();
    tiny.maximum_retained_bytes = r.retained_heap_bytes_bound - 1;
    assert!(
        resource::PreparedLexicalDictionary::prepare(inputs.clone(), "language/en", tiny).is_err()
    );
    let mut tiny = limits();
    tiny.maximum_preparation_peak_bytes = r.preparation_peak_heap_bytes_bound - 1;
    assert!(
        resource::PreparedLexicalDictionary::prepare(inputs.clone(), "language/en", tiny).is_err()
    );
    let mut malformed = inputs;
    malformed[0].push(0);
    assert!(
        resource::PreparedLexicalDictionary::prepare(malformed, "language/en", limits()).is_err()
    );
}

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);
static TRACK: AtomicBool = AtomicBool::new(false);
struct Probe;
unsafe impl GlobalAlloc for Probe {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = System.alloc(layout);
        if !pointer.is_null() {
            let live =
                LIVE.fetch_add(layout.size() as isize, Ordering::SeqCst) + layout.size() as isize;
            if TRACK.load(Ordering::SeqCst) {
                PEAK.fetch_max(live, Ordering::SeqCst);
            }
        }
        pointer
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::SeqCst);
        System.dealloc(pointer, layout);
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let result = System.realloc(pointer, layout, size);
        if !result.is_null() {
            let live = LIVE.fetch_add(size as isize - layout.size() as isize, Ordering::SeqCst)
                + size as isize
                - layout.size() as isize;
            if TRACK.load(Ordering::SeqCst) {
                PEAK.fetch_max(live, Ordering::SeqCst);
            }
        }
        result
    }
}
#[global_allocator]
static ALLOCATOR: Probe = Probe;
#[test]
fn resource_structural_peak_covers_measured_preparation_and_lookup() {
    let mut first = shard("record", "language/en");
    first.reserve(1024);
    let input = vec![first, shard("old", "language/en")];
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let mut prepared =
        resource::PreparedLexicalDictionary::prepare(input, "language/en", limits()).unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let observed = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    let receipt = prepared.storage_receipt();
    assert!(receipt.preparation_peak_heap_bytes_bound >= observed);
    let baseline = LIVE.load(Ordering::SeqCst);
    PEAK.store(baseline, Ordering::SeqCst);
    TRACK.store(true, Ordering::SeqCst);
    let result = prepared.lookup("record").unwrap().unwrap();
    TRACK.store(false, Ordering::SeqCst);
    let lookup = (PEAK.load(Ordering::SeqCst) - baseline).max(0) as usize;
    assert!(receipt.lookup_requested_bytes_bound >= lookup);
    assert_eq!(result.entry().surface(), "record");
    eprintln!("dictionary preparation_peak_observed={observed} lookup_peak_observed={lookup} receipt={receipt:?}");
}
