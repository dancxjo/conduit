use conduit_audio::{
    AudioRenderDemand, PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation, SoundInfoError,
};
use conduit_form::rust_binding::NativeBindingRefusal;

#[test]
fn render_demand_owns_the_exact_checked_interval_without_losing_domain_refusals() {
    let demand = AudioRenderDemand::new(7, u64::MAX - 240, 240, 2).unwrap();
    assert_eq!(AudioRenderDemand::decode(&demand.encode()), Ok(demand));
    assert_eq!(
        AudioRenderDemand::new(7, u64::MAX - 239, 240, 2),
        Err(SoundInfoError::OutOfRange("render-frame-interval"))
    );
    assert!(matches!(
        AudioRenderDemand::new_native(7, u64::MAX - 239, 240, 2),
        Err(NativeBindingRefusal::ViolatedInvariant { .. })
    ));
}

#[test]
fn pcm_header_derives_and_owns_every_exact_representation_width() {
    for (representation, layout, expected) in [
        (
            PcmSampleRepresentation::Signed16LittleEndian,
            PcmChannelLayout::Mono,
            8,
        ),
        (
            PcmSampleRepresentation::Signed24LittleEndian,
            PcmChannelLayout::StereoLeftRight,
            24,
        ),
        (
            PcmSampleRepresentation::Float32LittleEndian,
            PcmChannelLayout::StereoLeftRight,
            32,
        ),
    ] {
        let header = PcmFrameHeader::new(representation, 48_000, layout, 4, 9, 12, false).unwrap();
        assert_eq!(header.payload_bytes(), expected);
        assert_eq!(PcmFrameHeader::decode(&header.encode()), Ok(header));
        assert!(matches!(
            PcmFrameHeader::new_native(
                representation,
                48_000,
                layout,
                4,
                9,
                12,
                false,
                expected + 1
            ),
            Err(NativeBindingRefusal::ViolatedInvariant { .. })
        ));
    }
}
