use conduit_audio::*;
pub fn frame(samples: &[f32], rate: u32, start: u64) -> Vec<u8> {
    PcmFrameHeader::new(
        PcmSampleRepresentation::Float32LittleEndian,
        rate,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        55,
        start,
        false,
    )
    .unwrap()
    .encode_frame(
        &samples
            .iter()
            .flat_map(|x| x.to_le_bytes())
            .collect::<Vec<_>>(),
    )
    .unwrap()
}
pub fn signal(n: usize, period: f64, fundamental: f64, harmonic: f64) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let phase = std::f64::consts::TAU * i as f64 / period;
            (fundamental * phase.sin() + harmonic * (2.0 * phase).sin()) as f32
        })
        .collect()
}
