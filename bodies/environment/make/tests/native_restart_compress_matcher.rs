//! Independent pre-optimization encoder byte oracle; CPU codec proof only.
#![allow(dead_code)]
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress.rs"]
mod codec;
#[path = "../../../../targets/esp32/proof/tm1637/src/native_restart_compress_reference.rs"]
mod reference;
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
thread_local! {
    static TRACK: Cell<bool> = const { Cell::new(false) };
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}
struct Allocator;
unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        TRACK.with(|on| {
            if on.get() {
                ALLOCS.with(|n| n.set(n.get() + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) }
    }
}
#[global_allocator]
static ALLOCATOR: Allocator = Allocator;
fn encode(
    bytes: &[u8],
    widths: &[usize],
    old: bool,
    max_plain: usize,
    max_encoded: usize,
    fail: Option<usize>,
) -> (Vec<u8>, Result<(usize, usize), u8>) {
    let mut out = Vec::new();
    let mut calls = 0;
    let mut sink = |b: &[u8]| {
        let call = calls;
        calls += 1;
        if Some(call) == fail {
            return Err(());
        }
        out.extend_from_slice(b);
        Ok(())
    };
    macro_rules! run {
        ($encoder:ty) => {{
            let mut e = <$encoder>::new(&mut sink, max_plain, max_encoded);
            let mut at = 0;
            let mut index = 0;
            let result = loop {
                if at == bytes.len() {
                    break e
                        .finish()
                        .map(|n| (n.plain, n.encoded))
                        .map_err(|e| match e {
                            E::Capacity => 1,
                            E::Invalid => 2,
                        });
                }
                let n = widths[index % widths.len()].min(bytes.len() - at);
                index += 1;
                match e.write(&bytes[at..at + n]) {
                    Ok(()) => at += n,
                    Err(e) => {
                        break Err(match e {
                            E::Capacity => 1,
                            E::Invalid => 2,
                        })
                    }
                }
            };
            result
        }};
    }
    let result = if old {
        use reference::Error as E;
        run!(reference::Encoder<_>)
    } else {
        use codec::Error as E;
        run!(codec::Encoder<_>)
    };
    (out, result)
}
fn parity(bytes: &[u8], widths: &[usize]) {
    let old = encode(
        bytes,
        widths,
        true,
        bytes.len(),
        bytes.len() * 2 + 128,
        None,
    );
    let new = encode(
        bytes,
        widths,
        false,
        bytes.len(),
        bytes.len() * 2 + 128,
        None,
    );
    assert_eq!(new, old);
    assert!(new.1.is_ok());
}
fn random(len: usize, alphabet: u32) -> Vec<u8> {
    let mut state = 0x934c_119f_u32;
    (0..len)
        .map(|_| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            (state % alphabet) as u8
        })
        .collect()
}
#[test]
fn old_encoder_matches_all_small_write_partitions_and_short_finishes() {
    for len in 0_usize..=9 {
        let bytes: Vec<_> = (0..len).map(|i| b'a' + (i % 3) as u8).collect();
        for mask in 0..1_usize << len.saturating_sub(1) {
            let mut widths = Vec::new();
            let mut at = 0;
            for i in 1..len {
                if mask & (1 << (i - 1)) != 0 {
                    widths.push(i - at);
                    at = i;
                }
            }
            widths.push((len - at).max(1));
            parity(&bytes, &widths);
        }
    }
    for len in 1..=132 {
        parity(&vec![b'a'; len], &[1]);
    }
}
#[test]
fn old_encoder_matches_wrap_overlap_near_far_lazy_numeric_hex_and_cache() {
    let mut cases = vec![vec![b'a'; 6000], random(6000, 256), random(6000, 4)];
    let text = b"{\"name\":\"0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\",\"values\":[0,255,128,14,22,254],\"sequence\":1234567890}";
    cases.push(text.repeat(50));
    let mut far = random(1408, 256);
    far.extend_from_within(..65);
    far.extend_from_slice(b"aaaaaaaaabaaaaaaaaacaaaaaaaaab");
    cases.push(far);
    for n in [1407, 1408, 1409, 2815, 2816, 2817] {
        cases.push(random(n, 7));
    }
    for bytes in cases {
        for width in [1, 2, 3, 7, 64, 65, 127, 1024, usize::MAX] {
            parity(&bytes, &[width]);
        }
        parity(&bytes, &[1, 65, 2, 1408, 3, 64]);
    }
}
#[test]
fn old_encoder_matches_sink_failure_and_capacity_refusals() {
    let bytes = random(2049, 16);
    for fail in 0..8 {
        assert_eq!(
            encode(&bytes, &[7], false, 4096, 4096, Some(fail)),
            encode(&bytes, &[7], true, 4096, 4096, Some(fail))
        );
    }
    for (plain, encoded) in [
        (0, 4096),
        (2048, 4096),
        (4096, 0),
        (4096, 8),
        (4096, 32),
        (4096, 128),
    ] {
        assert_eq!(
            encode(&bytes, &[1, 64], false, plain, encoded, None),
            encode(&bytes, &[1, 64], true, plain, encoded, None)
        );
    }
}
#[test]
fn optimized_encoder_has_unchanged_inline_workspace_and_zero_allocations() {
    fn sink(_: &[u8]) -> Result<(), ()> {
        Ok(())
    }
    let mut e = codec::Encoder::new(sink as fn(&[u8]) -> Result<(), ()>, 4096, 8192);
    let old = reference::Encoder::new(sink as fn(&[u8]) -> Result<(), ()>, 4096, 8192);
    assert_eq!(core::mem::size_of_val(&e), core::mem::size_of_val(&old));
    let bytes = [b'a'; 2049];
    ALLOCS.with(|n| n.set(0));
    TRACK.with(|n| n.set(true));
    let written = e.write(&bytes);
    let finished = e.finish();
    TRACK.with(|n| n.set(false));
    assert!(written.is_ok() && finished.is_ok());
    assert_eq!(ALLOCS.with(Cell::get), 0);
}
fn private_stream(frame: &[u8], old: bool, width: usize) -> (Vec<u8>, usize) {
    let mut at = 0;
    let mut decoder = codec::Decoder::new(
        |out: &mut [u8; 128]| {
            let n = 128.min(frame.len() - at);
            out[..n].copy_from_slice(&frame[at..at + n]);
            at += n;
            Ok(n)
        },
        131072,
        32608,
    );
    let mut encoded = Vec::new();
    let mut sink = |bytes: &[u8]| {
        encoded.extend_from_slice(bytes);
        Ok(())
    };
    macro_rules! stream {
        ($encoder:ty) => {{
            let mut encoder = <$encoder>::new(&mut sink, 131072, 131072);
            let mut chunk = [0; 128];
            loop {
                let n = decoder.read(&mut chunk).unwrap();
                if n == 0 {
                    break;
                }
                for part in chunk[..n].chunks(width) {
                    encoder.write(part).unwrap();
                }
            }
            let length = encoder.finish().unwrap().plain;
            chunk.fill(0);
            length
        }};
    }
    let length = if old {
        stream!(reference::Encoder<_>)
    } else {
        stream!(codec::Encoder<_>)
    };
    (encoded, length)
}
#[test]
#[ignore = "private encoded frame path supplied explicitly; never logs its contents"]
fn private_complete_frame_whole_encoder_exact_bytes_and_timing() {
    let frame = std::fs::read(
        std::env::var_os("CONDUIT_PRIVATE_CODEC_FRAME").expect("explicit private encoded input"),
    )
    .unwrap();
    let start = std::time::Instant::now();
    let old = private_stream(&frame, true, 128);
    let old_us = start.elapsed().as_micros();
    let start = std::time::Instant::now();
    let new = private_stream(&frame, false, 128);
    let new_us = start.elapsed().as_micros();
    assert!(new == old);
    assert!(new.0 == frame);
    for width in [1, 2, 7, 65, 1024] {
        assert!(private_stream(&frame, false, width) == old);
    }
    println!("private_codec plain={} encoded={} reference_stream_us={} optimized_stream_us={} byte_equal=true",new.1,new.0.len(),old_us,new_us);
}
