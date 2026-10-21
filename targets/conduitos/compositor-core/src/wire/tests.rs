use super::*;
use std::vec;
const ENV: Envelope = Envelope {
    instance: u64::MAX,
    request: u64::MAX,
};
const HANDLE: Handle = Handle {
    slot: 7,
    generation: u64::MAX,
};
const RECT: Rect = Rect {
    x: i16::MIN,
    y: i16::MAX,
    width: 2048,
    height: 1024,
};
#[test]
fn every_request_roundtrips_and_rejects_truncation_or_trailing_bytes() {
    let requests = [
        Request::Admit { rect: RECT, z: 255 },
        Request::Remove(HANDLE),
        Request::Place {
            surface: HANDLE,
            x: i16::MIN,
            y: i16::MAX,
            z: 255,
        },
        Request::Visible {
            surface: HANDLE,
            visible: true,
        },
        Request::Focus(HANDLE),
        Request::Cursor {
            position: Some((i32::MAX as u32 - 10, i32::MAX as u32 - 16)),
            hover: true,
        },
        Request::Cursor {
            position: None,
            hover: false,
        },
        Request::BeginUpload {
            surface: HANDLE,
            revision: u64::MAX,
            rect: RECT,
        },
        Request::PrepareUpload {
            pixels: MAX_CHUNK as u32,
        },
        Request::UploadWindow {
            offset: MAX_PIXELS as u32 - MAX_CHUNK as u32,
            window_offset: 0,
            pixels: MAX_CHUNK as u32,
        },
        Request::UploadWindow {
            offset: 0,
            window_offset: OUTPUT_BYTES - 4,
            pixels: 1,
        },
        Request::CommitUpload,
        Request::AbortUpload,
        Request::BeginFrame,
        Request::FrameStep {
            sequence: u64::MAX,
            pixels: MAX_CHUNK as u32,
        },
        Request::AcknowledgeScanout { sequence: u64::MAX },
        Request::FailFrame,
        Request::Pointer {
            x: u32::MAX,
            y: u32::MAX,
        },
        Request::Keyboard,
        Request::Initialize {
            width: u16::MAX,
            height: u16::MAX,
        },
    ];
    for request in requests {
        let mut bytes = [0; EXCHANGE_BYTES];
        let length = encode_request(ENV, request, &mut bytes).unwrap();
        assert_eq!(decode_request(&bytes[..length]), Ok((ENV, request)));
        for truncated in 0..length {
            assert!(decode_request(&bytes[..truncated]).is_err());
        }
        // A valid header length must not permit unauthored trailing fields.
        bytes[length] = 0;
        bytes[8..12].copy_from_slice(&((length + 1) as u32).to_le_bytes());
        assert_eq!(
            decode_request(&bytes[..length + 1]),
            Err(CodecError::Length)
        );
    }
}
#[test]
fn request_header_and_payload_refuse_unknown_noncanonical_or_overflow_values() {
    let mut bytes = [0; EXCHANGE_BYTES];
    let length = encode_request(
        ENV,
        Request::Visible {
            surface: HANDLE,
            visible: true,
        },
        &mut bytes,
    )
    .unwrap();
    for (offset, value) in [(0, 0), (4, 2), (6, 255), (12, 1), (32, 8), (41, 2)] {
        let mut malformed = bytes;
        malformed[offset] = value;
        assert!(
            decode_request(&malformed[..length]).is_err(),
            "offset{offset}"
        );
    }
    for range in [16..24, 24..32, 33..41] {
        let mut malformed = bytes;
        malformed[range].fill(0);
        assert!(decode_request(&malformed[..length]).is_err());
    }
    let mut too_long = vec![0; EXCHANGE_BYTES + 1];
    too_long[..length].copy_from_slice(&bytes[..length]);
    assert_eq!(decode_request(&too_long), Err(CodecError::Length));
    let length = encode_request(
        ENV,
        Request::UploadWindow {
            offset: 0,
            window_offset: 0,
            pixels: 1,
        },
        &mut bytes,
    )
    .unwrap();
    for (offset, value) in [
        (32, u32::MAX),
        (36, 1),
        (36, OUTPUT_BYTES),
        (40, 0),
        (40, u32::MAX),
        (40, MAX_CHUNK as u32 + 1),
    ] {
        let mut malformed = bytes;
        malformed[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(decode_request(&malformed[..length]).is_err());
    }
    let length = encode_request(
        ENV,
        Request::Cursor {
            position: None,
            hover: false,
        },
        &mut bytes,
    )
    .unwrap();
    bytes[34..38].copy_from_slice(&1u32.to_le_bytes());
    assert_eq!(decode_request(&bytes[..length]), Err(CodecError::Value));
    let length = encode_request(
        ENV,
        Request::Initialize {
            width: 1,
            height: 1,
        },
        &mut bytes,
    )
    .unwrap();
    bytes[32..34].fill(0);
    assert!(decode_request(&bytes[..length]).is_err());
    assert!(
        encode_request(
            ENV,
            Request::FrameStep {
                sequence: 0,
                pixels: 1
            },
            &mut bytes
        )
        .is_err()
    );
    assert!(encode_request(ENV, Request::PrepareUpload { pixels: 0 }, &mut bytes).is_err());
    assert!(
        encode_request(
            ENV,
            Request::BeginUpload {
                surface: HANDLE,
                revision: 0,
                rect: RECT
            },
            &mut bytes
        )
        .is_err()
    );
}
#[test]
fn response_variants_roundtrip_and_frame_ranges_are_checked() {
    let frame = FrameSpan {
        sequence: u64::MAX,
        x: 0,
        y: 65534,
        window_offset: 0,
        pixels: MAX_CHUNK as u32,
        complete: true,
        coverage: [MAX_PIXELS as u32, 0, 0, 0, 0, 0, 0, 0],
    };
    let responses = [
        Response::Unit,
        Response::Surface(HANDLE),
        Response::Prepared(true),
        Response::FrameSequence(u64::MAX),
        Response::Frame(frame),
        Response::Pointer(None),
        Response::Pointer(Some(crate::Route {
            surface: HANDLE,
            revision: u64::MAX,
            x: u16::MAX,
            y: u16::MAX,
        })),
        Response::Keyboard(None),
        Response::Keyboard(Some((HANDLE, u64::MAX))),
        Response::Refused(Error::NoDamage),
        Response::Refused(Error::Exhausted),
    ];
    for response in responses {
        let mut bytes = [0; EXCHANGE_BYTES];
        let length = encode_response(ENV, response, &mut bytes).unwrap();
        assert_eq!(decode_response(&bytes[..length]), Ok((ENV, response)));
        for truncated in 0..length {
            assert!(decode_response(&bytes[..truncated]).is_err());
        }
        assert!(decode_request(&bytes[..length]).is_err());
    }
    for malformed in [
        FrameSpan {
            window_offset: 1,
            ..frame
        },
        FrameSpan { pixels: 0, ..frame },
        FrameSpan {
            complete: false,
            ..frame
        },
        FrameSpan {
            coverage: [u32::MAX; 8],
            ..frame
        },
        FrameSpan {
            x: u32::MAX,
            ..frame
        },
        FrameSpan { y: 65535, ..frame },
    ] {
        assert!(
            encode_response(ENV, Response::Frame(malformed), &mut [0; EXCHANGE_BYTES]).is_err()
        );
    }
    let mut bytes = [0; EXCHANGE_BYTES];
    let length = encode_response(ENV, Response::Frame(frame), &mut bytes).unwrap();
    bytes[56] = 2;
    assert_eq!(decode_response(&bytes[..length]), Err(CodecError::Value));
    let length = encode_response(ENV, Response::Refused(Error::Bounds), &mut bytes).unwrap();
    bytes[32..34].fill(255);
    assert_eq!(decode_response(&bytes[..length]), Err(CodecError::Value));
}
#[test]
fn pixel_window_is_little_endian_and_admits_exact_64kib_without_allocation() {
    let pixels = vec![0x1234_5678; MAX_CHUNK];
    let mut window = vec![0; OUTPUT_BYTES as usize];
    encode_pixels(&mut window, 0, &pixels).unwrap();
    assert_eq!(&window[..4], &[0x78, 0x56, 0x34, 0x12]);
    let decoded = decode_pixels(&window, 0, MAX_CHUNK as u32).unwrap();
    assert_eq!(decoded.pixels().count(), MAX_CHUNK);
    assert!(decoded.pixels().all(|p| p == 0x1234_5678));
    encode_pixels(&mut window, OUTPUT_BYTES - 4, &[u32::MAX]).unwrap();
    assert_eq!(
        decode_pixels(&window, OUTPUT_BYTES - 4, 1)
            .unwrap()
            .pixels()
            .next(),
        Some(u32::MAX)
    );
    let before = window.clone();
    for offset in [1, OUTPUT_BYTES, u32::MAX] {
        assert!(encode_pixels(&mut window, offset, &[1]).is_err());
        assert_eq!(window, before);
    }
    assert!(decode_pixels(&window, 0, 0).is_err());
    assert!(decode_pixels(&window, 0, u32::MAX).is_err());
    assert!(decode_pixels(&window[..3], 0, 1).is_err());
    assert!(encode_pixels(&mut window[..3], 0, &[1]).is_err());
}
