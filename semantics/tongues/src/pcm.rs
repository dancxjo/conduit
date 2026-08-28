use sha2::Digest;

pub(crate) fn deterministic_pcm(text: &str) -> Vec<u8> {
    let mut payload = Vec::with_capacity(text.len() * 64);
    for (index, byte) in text.bytes().enumerate() {
        for sample in 0..32u16 {
            let value = (i16::from(byte) - 64) * 128
                + i16::try_from((index + usize::from(sample)) % 32).unwrap() * 8;
            payload.extend_from_slice(&value.to_le_bytes());
        }
    }
    let frames = u16::try_from(payload.len() / 2).expect("bounded speech specimen frame count");
    conduit_audio::PcmFrameHeader::new(
        conduit_audio::PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        conduit_audio::PcmChannelLayout::Mono,
        frames,
        1,
        0,
        false,
    )
    .expect("fixed speech PCM profile")
    .encode_frame(&payload)
    .expect("fixed speech payload matches its profile")
}

pub(crate) fn wav(pcm: &[u8]) -> Vec<u8> {
    let (header, payload) = conduit_audio::PcmFrameHeader::decode_frame(pcm)
        .expect("audio/pcm-frames value must be canonical before WAV adaptation");
    let mut out = Vec::with_capacity(44 + payload.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + payload.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&u16::from(header.layout.channels()).to_le_bytes());
    out.extend_from_slice(&header.sample_rate_hz.to_le_bytes());
    let byte_rate = header.sample_rate_hz
        * u32::from(header.layout.channels())
        * u32::from(header.representation.bytes_per_sample());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align =
        u16::from(header.layout.channels()) * u16::from(header.representation.bytes_per_sample());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    out
}

pub(crate) fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}
