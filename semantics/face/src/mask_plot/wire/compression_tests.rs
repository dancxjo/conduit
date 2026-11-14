use super::*;
use miniz_oxide::deflate::compress_to_vec_zlib;

#[test]
fn bounded_encoder_matches_existing_engine_and_capacity_contract() {
    for size in [
        1,
        2,
        127,
        128,
        8192,
        16384,
        65500,
        65536,
        131072,
        MAX_EXPANDED_BYTES,
    ] {
        for compressible in [false, true] {
            let mut state = 0x1234_5678u32;
            let bytes = (0..size)
                .map(|index| {
                    if compressible {
                        (index % 17) as u8
                    } else {
                        state ^= state << 13;
                        state ^= state >> 17;
                        state ^= state << 5;
                        state as u8
                    }
                })
                .collect::<Vec<_>>();
            let ordinary = compress_to_vec_zlib(&bytes, 6);
            let bounded = bounded_compress(&bytes);
            if ordinary.len() <= MAX_COMPRESSED_BYTES {
                assert_eq!(bounded.as_deref(), Some(ordinary.as_slice()), "size={size}");
                assert_eq!(bounded.unwrap().capacity(), MAX_COMPRESSED_BYTES);
            } else {
                assert!(bounded.is_none(), "size={size}");
            }
        }
    }
}

#[test]
fn exact_compressed_limit_is_accepted_and_next_byte_is_refused() {
    // A fixed deterministic incompressible stream spans both sides of the
    // boundary. Compare actual encoder output, not an inferred raw-size limit.
    let mut state = 0x8765_4321u32;
    let input = (0..MAX_COMPRESSED_BYTES)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state as u8
        })
        .collect::<Vec<_>>();
    let mut exact = None;
    let mut next = None;
    for length in MAX_COMPRESSED_BYTES - 128..=MAX_COMPRESSED_BYTES {
        let ordinary = compress_to_vec_zlib(&input[..length], 6);
        if ordinary.len() == MAX_COMPRESSED_BYTES {
            exact = Some((length, ordinary));
        } else if ordinary.len() == MAX_COMPRESSED_BYTES + 1 {
            next = Some(length);
        }
    }
    let (length, ordinary) = exact.expect("fixture reaches exact compressed limit");
    assert_eq!(bounded_compress(&input[..length]).unwrap(), ordinary);
    assert!(bounded_compress(&input[..next.expect("fixture reaches next encoded byte")]).is_none());
}
