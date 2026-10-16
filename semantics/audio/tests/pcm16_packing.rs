use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

fn input(samples: &[i128], rate: u64, clock: u64, start: u64) -> Vec<u8> {
    AudioPcm16PackingInput::block(
        clock,
        false,
        rate,
        BoundedSequence::try_from_iter(samples.iter().copied()).unwrap(),
        start,
    )
    .unwrap()
    .encode()
    .unwrap()
}

#[test]
fn exact_pcm_header_and_signed_samples_round_trip() {
    let mut packer = PreparedPcm16Packing::prepare().unwrap();
    let encoded = input(&[-32768, -1, 0, 24, 32767], 8000, 17, 23);
    let output = packer.pack(&encoded).unwrap().unwrap();
    let (header, payload) = PcmFrameHeader::decode_frame(output).unwrap();
    assert_eq!(header.sample_rate_hz(), 8000);
    assert_eq!(header.clock_id(), 17);
    assert_eq!(header.start_frame(), 23);
    assert_eq!(header.frame_count(), 5);
    assert!(!header.discontinuity());
    let samples: Vec<_> = payload
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(samples, [-32768, -1, 0, 24, 32767]);
}

#[test]
fn refusal_never_clips_or_reuses_a_previous_block() {
    let mut packer = PreparedPcm16Packing::prepare().unwrap();
    assert!(packer.pack(&input(&[24], 8000, 1, 0)).unwrap().is_some());
    for sample in [-32769, 32768, i128::MAX] {
        assert!(matches!(
            packer.pack(&input(&[sample], 8000, 1, 0)),
            Err(Pcm16PackingRefusal::SampleOutOfRange)
        ));
    }
    assert!(packer.output().is_none());
    for (rate, clock, start) in [
        (7999, 1, 0),
        (192001, 1, 0),
        (8000, 0, 0),
        (8000, 1, u64::MAX),
    ] {
        assert!(matches!(
            packer.pack(&input(&[0], rate, clock, start)),
            Err(Pcm16PackingRefusal::InvalidHeader)
        ));
    }
    assert!(packer
        .pack(&AudioPcm16PackingInput::Skip.encode().unwrap())
        .unwrap()
        .is_none());
    assert!(packer.pack(&[0]).is_err());
    let maximum = input(&vec![32767; 1024], 192000, u64::MAX, 0);
    assert_eq!(
        packer.pack(&maximum).unwrap().unwrap().len(),
        PCM16_PACKING_MAXIMUM_OUTPUT_BYTES
    );
}
