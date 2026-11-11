//! Physical capture fixtures pass through checked Source and authored Forms.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_dht11_observation::{ObservationContract, PreparedSourceFrameDecoder};
#[test]
fn validated_capture_reaches_the_same_quantity_face_and_fahrenheit_mask() {
    use conduit_environment_face::PreparedObservationFace;
    use conduit_face::FaceBasis;
    use conduit_seven_segment_mask::{
        render, PreparedEnvironmentFace, SegmentPatternForm, SevenSegmentLook,
        SevenSegmentSelection,
    };
    let contract = ObservationContract::prepare().unwrap();
    let mut decoder = PreparedSourceFrameDecoder::prepare(&contract).unwrap();
    let basis = FaceBasis {
        body_id: None,
        wake_id: None,
        source_document_id: Some(PreparedSourceFrameDecoder::SOURCE_DOCUMENT_ID.into()),
        checked_plot_id: Some(PreparedSourceFrameDecoder::CHECKED_PLOT_ID.into()),
        expanded_plot_id: Some(PreparedSourceFrameDecoder::EXPANDED_PLOT_ID.into()),
        plan_id: None,
        active_play_id: None,
        sign_ids: vec![],
    };
    let mut face = PreparedObservationFace::prepare(basis, 64, 30_000).unwrap();
    let ((), measured) = allocation_probe::observe(|| {
        let result = decoder.decode(5, [53, 0, 20, 2, 75]).unwrap();
        face.publish_result(0, result, 1000).unwrap();
        let value = face.at(1000).unwrap();
        assert!(value.basis.plan_id.is_none());
        assert!(value.basis.active_play_id.is_none());
        let view =
            PreparedEnvironmentFace::prepare(value, conduit_environment_face::SUBJECT).unwrap();
        let shown = render(
            &view,
            SevenSegmentSelection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        assert_eq!(
            shown
                .show
                .digits()
                .map(|v| SegmentPatternForm::encode(v)[0]),
            [0x7d, 0x7f, 0x66, 0x71]
        );
        assert_eq!(*shown.show.decimal_points(), [false, true, false, false]);
    });
    assert_eq!((measured.allocations, measured.reallocations), (0, 0));
}
