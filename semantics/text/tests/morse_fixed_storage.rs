use conduit_text::*;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    static ALLOCATIONS: Cell<Option<usize>> = const { Cell::new(None) };
}
struct TrackedAllocator;
unsafe impl GlobalAlloc for TrackedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            if let Some(value) = count.get() {
                count.set(Some(value + 1));
            }
        });
        unsafe { System.realloc(pointer, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: TrackedAllocator = TrackedAllocator;

fn fixed_pipeline(
    text: &str,
    unit: u16,
    first: &mut [u8],
    second: &mut [u8],
) -> Result<usize, MorseError> {
    let length = morse_characters_from_text_into_slice(text, first)?;
    let length = morse_lookup_characters_into_slice(&first[..length], second)?;
    let length = morse_intersperse_gaps_into_slice(&second[..length], first)?;
    let length = morse_flatten_groups_into_slice(&first[..length], second)?;
    morse_symbols_to_pattern_into_slice(&second[..length], unit, first)
}

#[test]
fn fixed_stages_match_canonical_values_at_semantic_bounds() {
    for text in [
        "sos",
        "HELLO WORLD",
        "a b",
        "01234567890123456789012345678901",
        "00000000000000000000000000000000",
    ] {
        let mut first = [0xa5; MAXIMUM_MORSE_PATTERN_BYTES];
        let mut second = [0xa5; MAXIMUM_MORSE_PATTERN_BYTES];
        let characters = morse_characters_from_text(text).unwrap();
        let length = morse_characters_from_text_into_slice(text, &mut first).unwrap();
        assert_eq!(&first[..length], characters);
        assert!(first[length..].iter().all(|byte| *byte == 0xa5));
        let groups = morse_lookup_characters(&characters).unwrap();
        let length = morse_lookup_characters_into_slice(&first[..length], &mut second).unwrap();
        assert_eq!(&second[..length], groups);
        let gaps = morse_intersperse_gaps(&groups).unwrap();
        let length = morse_intersperse_gaps_into_slice(&second[..length], &mut first).unwrap();
        assert_eq!(&first[..length], gaps);
        let symbols = morse_flatten_groups(&gaps).unwrap();
        let length = morse_flatten_groups_into_slice(&first[..length], &mut second).unwrap();
        assert_eq!(&second[..length], symbols);
        for unit in [MINIMUM_MORSE_UNIT_MILLIS, 80, MAXIMUM_MORSE_UNIT_MILLIS] {
            let pattern = morse_symbols_to_pattern(&symbols, unit).unwrap();
            let length = morse_symbols_to_pattern_into_slice(&symbols, unit, &mut first).unwrap();
            assert_eq!(&first[..length], pattern);
        }
    }
}

#[test]
fn repeated_fixed_pipeline_performs_no_allocation() {
    let mut first = [0; MAXIMUM_MORSE_PATTERN_BYTES];
    let mut second = [0; MAXIMUM_MORSE_PATTERN_BYTES];
    ALLOCATIONS.with(|count| count.set(Some(0)));
    let mut result = Ok(0);
    for _ in 0..100 {
        result = fixed_pipeline(
            "00000000000000000000000000000000",
            80,
            &mut first,
            &mut second,
        );
    }
    let allocations = ALLOCATIONS.with(|count| count.replace(None).unwrap());
    assert!(result.is_ok());
    assert_eq!(allocations, 0);
}

#[test]
fn oversized_intermediates_refuse_without_growing_vec_storage() {
    let mut characters = vec![1, 33];
    characters.extend([b'A'; 33]);
    let mut groups = vec![1, 33];
    let mut gaps = vec![1, 33];
    for index in 0..33 {
        groups.extend_from_slice(&[5, 1, 1, 1, 1, 1]);
        gaps.extend_from_slice(&[if index == 0 { 0 } else { 3 }, 5, 1, 1, 1, 1, 1]);
    }
    for (input, capacity, encode) in [
        (
            characters.as_slice(),
            MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES,
            morse_lookup_characters_into as fn(&[u8], &mut Vec<u8>) -> Result<(), MorseError>,
        ),
        (
            groups.as_slice(),
            MAXIMUM_MORSE_GAPPED_GROUPS_BYTES,
            morse_intersperse_gaps_into,
        ),
        (
            gaps.as_slice(),
            MAXIMUM_MORSE_SYMBOLS_BYTES,
            morse_flatten_groups_into,
        ),
    ] {
        let mut output = Vec::with_capacity(capacity);
        output.push(0xa5);
        let before = output.capacity();
        assert_eq!(
            encode(input, &mut output),
            Err(MorseError::MalformedEncoding)
        );
        assert_eq!(output, [0xa5]);
        assert_eq!(output.capacity(), before);
    }
}

#[test]
fn fixed_storage_refuses_missing_capacity_and_preserves_semantic_errors() {
    let mut storage = [0xa5; MAXIMUM_MORSE_PATTERN_BYTES];
    assert_eq!(
        morse_characters_from_text_into_slice(
            "sos",
            &mut storage[..MAXIMUM_MORSE_CHARACTERS_BYTES - 1]
        ),
        Err(MorseError::OutputCapacity)
    );
    assert_eq!(
        morse_lookup_characters_into_slice(
            &[],
            &mut storage[..MAXIMUM_MORSE_SYMBOL_GROUPS_BYTES - 1]
        ),
        Err(MorseError::OutputCapacity)
    );
    assert_eq!(
        morse_intersperse_gaps_into_slice(
            &[],
            &mut storage[..MAXIMUM_MORSE_GAPPED_GROUPS_BYTES - 1]
        ),
        Err(MorseError::OutputCapacity)
    );
    assert_eq!(
        morse_flatten_groups_into_slice(&[], &mut storage[..MAXIMUM_MORSE_SYMBOLS_BYTES - 1]),
        Err(MorseError::OutputCapacity)
    );
    assert_eq!(
        morse_symbols_to_pattern_into_slice(
            &[],
            80,
            &mut storage[..MAXIMUM_MORSE_PATTERN_BYTES - 1]
        ),
        Err(MorseError::OutputCapacity)
    );
    assert!(storage.iter().all(|byte| *byte == 0xa5));
    for (text, expected) in [
        ("", MorseError::Empty),
        (" a", MorseError::InvalidWordGap),
        ("é", MorseError::UnsupportedCharacter),
    ] {
        assert_eq!(
            morse_characters_from_text_into_slice(text, &mut storage),
            Err(expected)
        );
    }
}
