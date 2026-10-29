use sha2::{Digest, Sha256};
use std::{hint::black_box, path::Path, time::Instant};
fn streamed(bytes: &[u8], lengths: &[usize]) -> [u8; 32] {
    let mut digest = Sha256::new();
    let mut at = 0;
    for length in lengths {
        digest.update(&bytes[at..at + length]);
        at += length;
    }
    digest.finalize().into()
}
#[inline(never)]
fn buffered<const N: usize>(bytes: &[u8], lengths: &[usize]) -> [u8; 32] {
    let mut digest = Sha256::new();
    let mut buffer = [0; N];
    let mut filled = 0;
    let mut at = 0;
    for &length in lengths {
        let mut input = &bytes[at..at + length];
        at += length;
        while !input.is_empty() {
            let copied = input.len().min(N - filled);
            buffer[filled..filled + copied].copy_from_slice(&input[..copied]);
            filled += copied;
            input = &input[copied..];
            if filled == N {
                digest.update(buffer);
                filled = 0;
            }
        }
    }
    digest.update(&buffer[..filled]);
    digest.finalize().into()
}
fn measure(label: &str, mut operation: impl FnMut() -> [u8; 32]) {
    let clock = Instant::now();
    for _ in 0..2000 {
        black_box(operation());
    }
    println!("{label} ns_per_hash={}", clock.elapsed().as_nanos() / 2000);
}
fn main() {
    let directory = std::env::args().nth(1).unwrap();
    let directory = Path::new(&directory);
    let bytes = std::fs::read(directory.join("canonical-transcript.bin")).unwrap();
    let lengths = std::fs::read_to_string(directory.join("canonical-chunk-lengths.json")).unwrap();
    let lengths: Vec<usize> = lengths
        .trim_matches(['[', ']'])
        .split(',')
        .map(|v| v.parse().unwrap())
        .collect();
    assert_eq!(lengths.iter().sum::<usize>(), bytes.len());
    let expected: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(streamed(&bytes, &lengths), expected);
    assert_eq!(buffered::<64>(&bytes, &lengths), expected);
    assert_eq!(buffered::<256>(&bytes, &lengths), expected);
    // Arbitrary UTF-8, empty chunks, binary/floating bit patterns and boundaries.
    for size in [0, 1, 63, 64, 65, 255, 256, 257, 1024] {
        let edge: Vec<u8> = "α\0🦀"
            .as_bytes()
            .iter()
            .copied()
            .chain([0, 0, 0, 0, 0, 0, 0xf8, 0x7f])
            .cycle()
            .take(size)
            .collect();
        let chunks = [0, size / 2, 0, size - size / 2, 0];
        assert_eq!(streamed(&edge, &chunks), buffered::<64>(&edge, &chunks));
        assert_eq!(streamed(&edge, &chunks), buffered::<256>(&edge, &chunks));
    }
    println!(
        "calls={} bytes={} sha256={:x}",
        lengths.len(),
        bytes.len(),
        Sha256::digest(&bytes)
    );
    measure("streamed", || {
        streamed(black_box(&bytes), black_box(&lengths))
    });
    measure("buffer64", || {
        buffered::<64>(black_box(&bytes), black_box(&lengths))
    });
    measure("buffer256", || {
        buffered::<256>(black_box(&bytes), black_box(&lengths))
    });
}
