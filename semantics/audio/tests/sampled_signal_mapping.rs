use conduit_audio::*;
use conduit_data::{SignalCadence, SignalContinuity, SignalStart, TensorAxisRole, TensorElement};

#[test]
fn every_pcm_representation_maps_losslessly_to_the_generic_clock_contract() {
    for (representation, element) in [
        (
            PcmSampleRepresentation::Signed16LittleEndian,
            TensorElement::I16,
        ),
        (
            PcmSampleRepresentation::Signed24LittleEndian,
            TensorElement::I24,
        ),
        (
            PcmSampleRepresentation::Float32LittleEndian,
            TensorElement::F32,
        ),
    ] {
        let header = PcmFrameHeader::new(
            representation,
            48_000,
            PcmChannelLayout::StereoLeftRight,
            4,
            17,
            120,
            true,
        )
        .unwrap();
        let payload = vec![7; header.payload_bytes as usize];
        let signal = pcm_as_sampled_signal(header, &payload).unwrap();
        signal.validate().unwrap();
        assert_eq!(signal.clock_identity, "audio/pcm-clock/17");
        assert_eq!(signal.start, SignalStart::SampleIndex(120));
        assert_eq!(
            signal.cadence,
            SignalCadence::Regular {
                samples: 48_000,
                per: conduit_core::Quantity::new(1, conduit_core::QuantityUnit::Second)
            }
        );
        assert_eq!(signal.samples.element, element);
        assert_eq!(signal.samples.axes[0].role, TensorAxisRole::Time);
        assert!(matches!(
            signal.continuity,
            SignalContinuity::Discontinuous { .. }
        ));
        let (recovered_header, recovered_payload) = sampled_signal_as_pcm(&signal).unwrap();
        assert_eq!(recovered_header, header);
        assert_eq!(recovered_payload, payload);
    }
}
