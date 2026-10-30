use conduit_audio::*;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;

fn frame(clock: u64, start: u64, rate: u32, discontinuity: bool, samples: &[i16]) -> Vec<u8> {
    let payload: Vec<_> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        clock,
        start,
        discontinuity,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}
// Intentionally assemble malformed sequences without the production encoder's
// validation, so no invalid tail can hide behind a successful prefix.
fn wire(frames: &[&[u8]], declared: u32) -> Vec<u8> {
    let mut bytes = b"CDTPCM01".to_vec();
    bytes.extend_from_slice(&(frames.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&declared.to_le_bytes());
    for frame in frames {
        bytes.extend_from_slice(&(frame.len() as u32).to_le_bytes());
        bytes.extend_from_slice(frame);
    }
    bytes
}

#[test]
fn original_frames_are_borrowed_with_exact_clock_offset_and_cursor_extent() {
    let first = frame(7, 20, 16000, false, &[1, 2, 3]);
    let second = frame(7, 23, 16000, false, &[4, 5]);
    let encoded = encode_pcm_clip(&[&first, &second]).unwrap();
    let original = encoded.clone();
    let view = PcmClipView::decode(&encoded).unwrap();
    assert_eq!((view.block_count(), view.frame_count()), (2, 5));
    assert_eq!(*view.profile().clock_id(), 7);
    let mut cursor = view.encoded_frames();
    assert_eq!(cursor.len(), 2);
    let block = cursor.next().unwrap();
    assert_eq!(block, first);
    assert_eq!(block.as_ptr(), encoded[18..].as_ptr());
    assert_eq!(cursor.len(), 1);
    let fork = cursor.clone();
    assert_eq!(cursor.next().unwrap(), second);
    assert_eq!(fork.count(), 1);
    assert_eq!(cursor.len(), 0);
    assert!(cursor.next().is_none());
    assert!(cursor.next().is_none());
    let decoded = decode_pcm_clip(&encoded).unwrap();
    assert_eq!(view.frames().collect::<Vec<_>>(), decoded.blocks);
    assert_eq!(view.frames().next().unwrap().header.start_frame(), 20);
    assert_eq!(encoded, original);
}

#[test]
fn complete_semantic_sequence_is_checked_before_any_frame_is_exposed() {
    let first = frame(7, 0, 16000, false, &[1, 2]);
    for (tail, refusal) in [
        (frame(8, 2, 16000, false, &[3]), PcmClipError::MixedClock),
        (frame(7, 3, 16000, false, &[3]), PcmClipError::NonContiguous),
        (frame(7, 2, 8000, false, &[3]), PcmClipError::MixedProfile),
        (frame(7, 2, 16000, true, &[3]), PcmClipError::Discontinuity),
    ] {
        let bytes = wire(&[&first, &tail], 3);
        assert_eq!(PcmClipView::decode(&bytes).unwrap_err(), refusal);
        assert_eq!(decode_pcm_clip(&bytes).unwrap_err(), refusal);
    }
    assert_eq!(
        PcmClipView::decode(&wire(&[&first], 3)).unwrap_err(),
        PcmClipError::DeclaredExtentMismatch
    );
    let overflow = frame(7, u64::MAX, 16000, false, &[1]);
    assert_eq!(
        PcmClipView::decode(&wire(&[&overflow], 1)).unwrap_err(),
        PcmClipError::FrameCountOverflow
    );
}

#[test]
fn malformed_envelope_and_full_extent_limits_refuse_without_a_partial_view() {
    let first = frame(7, 0, 16000, false, &[1, 2]);
    let wrong_clock = frame(8, 2, 16000, false, &[3]);
    let mut truncated = wire(&[&first, &wrong_clock, &first], 5);
    truncated.pop();
    assert_eq!(
        PcmClipView::decode(&truncated).unwrap_err(),
        PcmClipError::MalformedFrame
    );
    let mut trailing = wire(&[&first, &wrong_clock], 3);
    trailing.push(0);
    assert_eq!(
        PcmClipView::decode(&trailing).unwrap_err(),
        PcmClipError::TrailingBytes
    );
    assert_eq!(
        PcmClipView::decode(&wire(&[], 0)).unwrap_err(),
        PcmClipError::Empty
    );
    let too_many: Vec<_> = (0..65).map(|_| first.as_slice()).collect();
    assert_eq!(
        PcmClipView::decode(&wire(&too_many, 130)).unwrap_err(),
        PcmClipError::TooManyBlocks
    );
    assert_eq!(
        PcmClipView::decode(&vec![0; MAXIMUM_PCM_CLIP_BYTES + 1]).unwrap_err(),
        PcmClipError::BoundExceeded
    );
    let frames: Vec<_> = (0..47)
        .map(|i| frame(7, i * 2048, 16000, false, &[0; 2048]))
        .collect();
    let slices: Vec<_> = frames.iter().map(Vec::as_slice).collect();
    assert_eq!(
        PcmClipView::decode(&wire(&slices, 47 * 2048)).unwrap_err(),
        PcmClipError::BoundExceeded
    );
}

#[test]
fn maximum_block_view_validation_and_repeated_iteration_allocate_nothing() {
    let frames: Vec<_> = (0..64)
        .map(|i| frame(7, 100 + i * 2, 16000, false, &[1, 2]))
        .collect();
    let slices: Vec<_> = frames.iter().map(Vec::as_slice).collect();
    let encoded = encode_pcm_clip(&slices).unwrap();
    let (extent, storage) = allocation_probe::observe(|| {
        let view = PcmClipView::decode(&encoded).unwrap();
        let count: usize = (0..4)
            .map(|_| view.encoded_frames().map(<[u8]>::len).sum::<usize>())
            .sum();
        let total: u32 = view
            .frames()
            .map(|f| u32::from(f.header.frame_count()))
            .sum();
        (count, total)
    });
    assert_eq!(
        extent,
        (4 * frames.iter().map(Vec::len).sum::<usize>(), 128)
    );
    assert_eq!(
        (
            storage.allocations,
            storage.reallocations,
            storage.deallocations
        ),
        (0, 0, 0)
    );
}

#[test]
fn borrowed_clip_feeds_existing_waveform_and_fft_without_copying_or_allocating() {
    let first = frame(7, 20, 16000, false, &[16384; 16]);
    let second = frame(7, 36, 16000, false, &[-8192; 16]);
    let encoded = encode_pcm_clip(&[&first, &second]).unwrap();
    let mut fft = PcmSpectralWorkspace::default();
    let ((rms, dc), storage) = allocation_probe::observe(|| {
        let view = PcmClipView::decode(&encoded).unwrap();
        let mut rms = 0.0;
        let mut dc = 0.0;
        for (i, block) in view.encoded_frames().enumerate() {
            let time = PreparedPcmTimeAnalysis::prepare(block, 7, 16000, 0, 16).unwrap();
            let spectrum = fft
                .analyze(block, 7, 16000, 0, 16, PcmSpectralWindow::Rectangular)
                .unwrap();
            assert_eq!(time.interval(), spectrum.interval());
            assert_eq!(time.interval().start_frame, 20 + i as u64 * 16);
            assert_eq!(time.source_digest(), spectrum.source_digest());
            rms += time.statistics().rms;
            dc += spectrum.bins().next().unwrap().peak_amplitude;
        }
        (rms, dc)
    });
    assert_eq!((rms, dc), (0.75, 0.75));
    assert_eq!(
        (
            storage.allocations,
            storage.reallocations,
            storage.deallocations
        ),
        (0, 0, 0)
    );
}

#[test]
fn malformed_or_foreign_tail_refusal_also_allocates_nothing() {
    let first = frame(7, 0, 16000, false, &[1, 2]);
    let foreign = frame(8, 2, 16000, false, &[3]);
    let wrong_clock = wire(&[&first, &foreign], 3);
    let wrong_extent = wire(&[&first], 3);
    let mut truncated = wire(&[&first, &foreign], 3);
    truncated.pop();
    let cases = [
        (wrong_clock, PcmClipError::MixedClock),
        (wrong_extent, PcmClipError::DeclaredExtentMismatch),
        (truncated, PcmClipError::MalformedFrame),
    ];
    let (_, storage) = allocation_probe::observe(|| {
        for (bytes, refusal) in &cases {
            assert_eq!(PcmClipView::decode(bytes).err(), Some(*refusal));
        }
    });
    assert_eq!(
        (
            storage.allocations,
            storage.reallocations,
            storage.deallocations
        ),
        (0, 0, 0)
    );
}
