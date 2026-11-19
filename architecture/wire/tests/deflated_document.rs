#[cfg(feature = "deflate")]
use conduit_wire::DeflatedDocumentLimits as Limits;
use conduit_wire::{DeflatedDocument as Document, DeflatedDocumentError as E};
#[cfg(feature = "deflate")]
#[path = "../../plot/tests/common/allocation_probe.rs"]
mod probe;
#[cfg(feature = "deflate")]
#[global_allocator]
static ALLOC: probe::Allocator = probe::Allocator;

// An independently encoded zlib stream for "hello".
const ZLIB: &[u8] = &[120, 156, 203, 72, 205, 201, 201, 7, 0, 6, 44, 2, 21];

#[test]
fn image_header_is_exact_and_does_not_require_the_decoder() {
    let document = Document::from_parts(ZLIB, 5).unwrap();
    let mut image = [0; 21];
    assert_eq!(document.write_header(&mut image).unwrap(), 8);
    image[8..].copy_from_slice(ZLIB);
    assert_eq!(&image[..8], b"CDZ1\x05\x00\x00\x00");
    let parsed = Document::parse_image(&image).unwrap();
    assert_eq!(parsed.expanded_bytes(), 5);
    assert_eq!(parsed.compressed(), ZLIB);
    assert_eq!(parsed.compressed().as_ptr(), image[8..].as_ptr());
    let mut short = [0xa5; 7];
    assert_eq!(document.write_header(&mut short), Err(E::Bounds));
    assert_eq!(short, [0xa5; 7]);
    assert!(matches!(Document::from_parts(&[], 5), Err(E::Bounds)));
    assert!(matches!(Document::from_parts(ZLIB, 0), Err(E::Bounds)));
    for length in 0..=8 {
        assert!(matches!(
            Document::parse_image(&image[..length]),
            Err(E::Header)
        ));
    }
    image[3] = b'2';
    assert!(matches!(Document::parse_image(&image), Err(E::Header)));
}

#[cfg(feature = "deflate")]
fn limits() -> Limits {
    Limits {
        maximum_expanded_bytes: 5,
        maximum_compressed_bytes: ZLIB.len(),
    }
}

#[test]
#[cfg(feature = "deflate")]
fn decode_uses_only_the_admitted_output_and_allocates_nothing() {
    let document = Document::from_parts(ZLIB, 5).unwrap();
    let mut output = [0xa5; 9];
    let ((), cost) = probe::observe(|| {
        let raw = document.decode_into(&mut output, limits()).unwrap();
        assert_eq!(raw, b"hello");
    });
    assert_eq!(cost.allocations, 0);
    assert_eq!(cost.reallocations, 0);
    assert_eq!(&output[5..], &[0xa5; 4]);
    for (length, local) in [
        (4, limits()),
        (
            9,
            Limits {
                maximum_expanded_bytes: 4,
                ..limits()
            },
        ),
        (
            9,
            Limits {
                maximum_compressed_bytes: ZLIB.len() - 1,
                ..limits()
            },
        ),
    ] {
        let mut output = [0xa5; 9];
        assert_eq!(
            document.decode_into(&mut output[..length], local),
            Err(E::Bounds)
        );
        assert_eq!(output, [0xa5; 9]);
    }
    let document = Document::from_parts(ZLIB, u32::MAX as usize).unwrap();
    assert_eq!(document.decode_into(&mut output, limits()), Err(E::Bounds));
}

#[test]
#[cfg(feature = "deflate")]
fn suffixes_concatenation_truncation_wrong_lengths_and_bad_checksums_refuse() {
    let mut cases = Vec::new();
    cases.push([ZLIB, &[0]].concat());
    cases.push([ZLIB, ZLIB].concat());
    for length in 1..ZLIB.len() {
        cases.push(ZLIB[..length].to_vec());
    }
    let mut bad = ZLIB.to_vec();
    bad[0] ^= 1;
    cases.push(bad);
    let mut bad = ZLIB.to_vec();
    *bad.last_mut().unwrap() ^= 1;
    cases.push(bad);
    for bytes in cases {
        let document = Document::from_parts(&bytes, 5).unwrap();
        assert_eq!(
            document.decode_into(
                &mut [0; 5],
                Limits {
                    maximum_expanded_bytes: 5,
                    maximum_compressed_bytes: 2 * ZLIB.len(),
                }
            ),
            Err(E::Stream)
        );
    }
    for extent in [1, 4, 6] {
        let document = Document::from_parts(ZLIB, extent).unwrap();
        assert_eq!(
            document.decode_into(
                &mut [0; 6],
                Limits {
                    maximum_expanded_bytes: 6,
                    ..limits()
                }
            ),
            Err(E::Stream)
        );
    }
}
