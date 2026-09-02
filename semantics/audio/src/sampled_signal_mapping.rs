//! Lossless semantic mapping from compact PCM blocks to generic sampled signals.

use alloc::{format, string::ToString, vec};
use conduit_core::{Quantity, QuantityUnit};
use conduit_data::{
    tensor_content_digest, SampledSignal, SignalCadence, SignalContinuity, SignalStart, TensorAxis,
    TensorAxisRole, TensorBacking, TensorElement, TensorValue,
};

use crate::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, SoundInfoError};

pub fn pcm_as_sampled_signal(
    header: PcmFrameHeader,
    payload: &[u8],
) -> Result<SampledSignal, SoundInfoError> {
    header.validate_payload(payload)?;
    let channels = u64::from(header.layout.channels());
    let element = match header.representation {
        PcmSampleRepresentation::Signed16LittleEndian => TensorElement::I16,
        PcmSampleRepresentation::Signed24LittleEndian => TensorElement::I24,
        PcmSampleRepresentation::Float32LittleEndian => TensorElement::F32,
    };
    Ok(SampledSignal {
        clock_identity: format!("audio/pcm-clock/{}", header.clock_id),
        start: SignalStart::SampleIndex(header.start_frame),
        cadence: SignalCadence::Regular {
            samples: u64::from(header.sample_rate_hz),
            per: Quantity::new(1, QuantityUnit::Second),
        },
        sample_count: u64::from(header.frame_count),
        continuity: if header.discontinuity {
            SignalContinuity::Discontinuous {
                gap_identity: "audio/declared-discontinuity".to_string(),
            }
        } else {
            SignalContinuity::Continuous
        },
        samples: TensorValue {
            element,
            dimensions: vec![u64::from(header.frame_count), channels],
            axes: vec![
                TensorAxis {
                    role: TensorAxisRole::Time,
                    identity: Some("pcm-frame".to_string()),
                    unit: None,
                },
                TensorAxis {
                    role: TensorAxisRole::Channel,
                    identity: Some(
                        match header.layout {
                            PcmChannelLayout::Mono => "mono",
                            PcmChannelLayout::StereoLeftRight => "stereo-left-right",
                        }
                        .to_string(),
                    ),
                    unit: Some(QuantityUnit::One),
                },
            ],
            content_digest: tensor_content_digest(payload),
            backing: TensorBacking::Inline(payload.to_vec()),
        },
    })
}

/// Recovers the compact PCM header and exact inline bytes from its generic
/// semantic representation. Signals outside the PCM profile refuse instead of
/// silently inventing an audio layout, clock, or representation.
pub fn sampled_signal_as_pcm(
    signal: &SampledSignal,
) -> Result<(PcmFrameHeader, &[u8]), SoundInfoError> {
    signal
        .validate()
        .map_err(|_| SoundInfoError::OutOfRange("sampled-signal"))?;
    let clock_id = signal
        .clock_identity
        .strip_prefix("audio/pcm-clock/")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value != 0)
        .ok_or(SoundInfoError::OutOfRange("pcm-clock-identity"))?;
    let SignalStart::SampleIndex(start_frame) = signal.start else {
        return Err(SoundInfoError::OutOfRange("pcm-start"));
    };
    let SignalCadence::Regular { samples, per } = signal.cadence else {
        return Err(SoundInfoError::OutOfRange("pcm-cadence"));
    };
    if per != Quantity::new(1, QuantityUnit::Second) {
        return Err(SoundInfoError::OutOfRange("pcm-cadence"));
    }
    let sample_rate_hz =
        u32::try_from(samples).map_err(|_| SoundInfoError::OutOfRange("sample-rate-hz"))?;
    let frame_count = u16::try_from(signal.sample_count)
        .map_err(|_| SoundInfoError::OutOfRange("frame-count"))?;
    let representation = match signal.samples.element {
        TensorElement::I16 => PcmSampleRepresentation::Signed16LittleEndian,
        TensorElement::I24 => PcmSampleRepresentation::Signed24LittleEndian,
        TensorElement::F32 => PcmSampleRepresentation::Float32LittleEndian,
        _ => return Err(SoundInfoError::OutOfRange("pcm-sample-representation")),
    };
    let layout = match signal.samples.dimensions.as_slice() {
        [_, 1] => PcmChannelLayout::Mono,
        [_, 2] => PcmChannelLayout::StereoLeftRight,
        _ => return Err(SoundInfoError::OutOfRange("pcm-channel-layout")),
    };
    let discontinuity = match &signal.continuity {
        SignalContinuity::Continuous => false,
        SignalContinuity::Discontinuous { gap_identity }
            if gap_identity == "audio/declared-discontinuity" =>
        {
            true
        }
        _ => return Err(SoundInfoError::OutOfRange("pcm-continuity")),
    };
    let TensorBacking::Inline(payload) = &signal.samples.backing else {
        return Err(SoundInfoError::OutOfRange("pcm-inline-payload"));
    };
    let header = PcmFrameHeader::new(
        representation,
        sample_rate_hz,
        layout,
        frame_count,
        clock_id,
        start_frame,
        discontinuity,
    )?;
    header.validate_payload(payload)?;
    Ok((header, payload))
}
