use conduit_core::{
    kind_id, CheckedValueContract, Quantity, Unit, QUANTITY_ENCODED_LEN, TEMPERATURE_INFO_ID,
};
use conduit_face::{Face, FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
use conduit_seven_segment_mask::*;

#[test]
fn tiny_affine_temperatures_round_through_the_shared_mask_path() {
    for coefficient in [-1, 1] {
        let quantity = Quantity::from_decimal(coefficient, -128, Unit::Celsius).unwrap();
        let property = |name: &str, value| FaceProperty {
            subject: "environment".into(),
            name: name.into(),
            value,
        };
        let face = Face::new(
            7,
            FaceBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![FaceSubject {
                identity: "environment".into(),
                role: FaceRole::Info,
                name: "Temperature".into(),
            }],
            vec![],
            vec![
                property(
                    "temperature",
                    FacePropertyValue::TypedValue {
                        contract: CheckedValueContract::new(
                            kind_id(TEMPERATURE_INFO_ID),
                            QUANTITY_ENCODED_LEN as u32,
                            vec![],
                        )
                        .unwrap(),
                        bytes: quantity.encode().to_vec(),
                    },
                ),
                property("fresh", FacePropertyValue::Flag(true)),
                property("sensor-status", FacePropertyValue::Text("available".into())),
            ],
            vec![],
        )
        .unwrap();
        let before = face.clone();
        let view = PreparedEnvironmentFace::prepare(&face, "environment").unwrap();
        let output = render(
            &view,
            SevenSegmentSelection::Temperature,
            &SevenSegmentLook::fahrenheit(),
        );
        let glyphs = output
            .show
            .digits()
            .map(|digit| SegmentPatternForm::encode(digit)[0]);
        assert_eq!(
            glyphs,
            [0x4f, 0x5b, 0x3f, 0x71],
            "temperature must display32.0F"
        );
        assert_eq!(*output.show.decimal_points(), [false, true, false, false]);
        assert!(!*output.show.colon());
        assert_eq!(
            output.disposition,
            MaskDisposition::Temperature { decimal_places: 1 }
        );
        assert_eq!(output.face_content_id, face.identity.as_str());
        assert_eq!(face, before);
    }
}
