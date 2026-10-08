//! Bounded volatile compiler support. Aligned words reduce private-state setup
//! work without reading or writing outside the caller's exact byte interval.
const WORD: usize = core::mem::size_of::<usize>();

/// Caller supplies valid intervals; forward-safe overlap is also supported.
pub(crate) unsafe fn copy_forward(destination: *mut u8, source: *const u8, length: usize) {
    let mut offset = 0;
    while offset < length
        && ((destination as usize + offset) | (source as usize + offset)) & (WORD - 1) != 0
    {
        unsafe {
            destination
                .add(offset)
                .write_volatile(source.add(offset).read_volatile());
        }
        offset += 1;
    }
    while length - offset >= WORD {
        unsafe {
            destination
                .add(offset)
                .cast::<usize>()
                .write_volatile(source.add(offset).cast::<usize>().read_volatile());
        }
        offset += WORD;
    }
    while offset < length {
        unsafe {
            destination
                .add(offset)
                .write_volatile(source.add(offset).read_volatile());
        }
        offset += 1;
    }
}

/// Caller supplies a valid writable interval. Padding and tails remain byte exact.
pub(crate) unsafe fn fill(destination: *mut u8, value: u8, length: usize) {
    let mut offset = 0;
    while offset < length && (destination as usize + offset) & (WORD - 1) != 0 {
        unsafe {
            destination.add(offset).write_volatile(value);
        }
        offset += 1;
    }
    let packed = usize::from(value).wrapping_mul(usize::MAX / 255);
    while length - offset >= WORD {
        unsafe {
            destination
                .add(offset)
                .cast::<usize>()
                .write_volatile(packed);
        }
        offset += WORD;
    }
    while offset < length {
        unsafe {
            destination.add(offset).write_volatile(value);
        }
        offset += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const CAPACITY: usize = 16_384 + 2 * 16;

    #[test]
    fn tour_timer_domain_copy_preserves_all_alignment_tails_and_canaries() {
        let source: [u8; CAPACITY] = core::array::from_fn(|index| (index.wrapping_mul(37)) as u8);
        for source_offset in 16..16 + WORD {
            for destination_offset in 16..16 + WORD {
                for length in (0..=96).chain([127, 128, 255, 256, 4095, 4096, 16_384]) {
                    let mut actual = [0xa5; CAPACITY];
                    let mut expected = actual;
                    expected[destination_offset..destination_offset + length]
                        .copy_from_slice(&source[source_offset..source_offset + length]);
                    unsafe {
                        copy_forward(
                            actual.as_mut_ptr().add(destination_offset),
                            source.as_ptr().add(source_offset),
                            length,
                        );
                    }
                    assert_eq!(
                        actual, expected,
                        "source={source_offset} destination={destination_offset} length={length}"
                    );
                }
            }
        }
    }

    #[test]
    fn tour_timer_domain_fill_preserves_all_alignment_tails_and_canaries() {
        for offset in 16..16 + WORD {
            for value in [0, 1, 0x80, 0xff] {
                for length in (0..=96).chain([127, 128, 255, 256, 4095, 4096, 16_384]) {
                    let mut actual = [0xa5; CAPACITY];
                    let mut expected = actual;
                    expected[offset..offset + length].fill(value);
                    unsafe {
                        fill(actual.as_mut_ptr().add(offset), value, length);
                    }
                    assert_eq!(
                        actual, expected,
                        "offset={offset} value={value} length={length}"
                    );
                }
            }
        }
    }
}
