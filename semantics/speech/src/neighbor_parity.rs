//! Authored neighbor endpoint selection retains exact stop-place cues.
use crate::{
    frame_parity::{field_type, integers, record},
    generated::*,
    onset_parity::variant,
    prosody_parity::{evaluate, graph},
};

#[test]
fn both_neighbor_endpoints_match_portable_graph_and_scalar_composition() {
    let (programs, result) = graph("speech_neighbor_endpoint");
    assert_eq!(programs.len(), 7);
    let input_type = &programs[0].1.input_type;
    let output_type = &programs[result].1.output_type;
    for (phone, tag) in PHONES {
        for (side, side_tag) in [
            (SpeechEndpointSide::start, "start"),
            (SpeechEndpointSide::end, "end"),
        ] {
            let mut target = speech_voice_target(*phone).unwrap();
            for frames in [target.frames, i32::MIN, i32::MAX] {
                target.frames = frames;
                let value = SpeechNeighborEndpointInput {
                    phone: *phone,
                    target,
                    side,
                };
                let encoded = record(
                    input_type,
                    &[
                        ("phone", variant(field_type(input_type, "phone"), tag)),
                        (
                            "target",
                            integers(field_type(input_type, "target"), &target_fields(target)),
                        ),
                        ("side", variant(field_type(input_type, "side"), side_tag)),
                    ],
                )
                .canonical_bytes()
                .unwrap();
                let actual = speech_neighbor_endpoint(value);
                let scalar = (if matches!(side, SpeechEndpointSide::end) {
                    frames.checked_sub(1)
                } else {
                    Some(0)
                })
                .and_then(|frame| {
                    speech_phone_frame_target(SpeechTrajectoryInput {
                        phone: *phone,
                        target,
                        frame,
                    })
                })
                .map(|model| SpeechNeighborEndpointResult {
                    neighbor: SpeechNeighborModel {
                        relation: SpeechNeighborRelation::segment,
                        model,
                    },
                    place: speech_stop_place(*phone).unwrap(),
                });
                assert_eq!(actual, scalar, "{phone:?}/{side:?}/{frames}");
                let expected = actual.map(|out| {
                    let neighbor_type = field_type(output_type, "neighbor");
                    let place_tag = match out.place {
                        SpeechStopPlace::labial => "labial",
                        SpeechStopPlace::alveolar => "alveolar",
                        SpeechStopPlace::velar => "velar",
                        SpeechStopPlace::not_stop => "not_stop",
                    };
                    record(
                        output_type,
                        &[
                            (
                                "neighbor",
                                record(
                                    neighbor_type,
                                    &[
                                        (
                                            "relation",
                                            variant(
                                                field_type(neighbor_type, "relation"),
                                                "segment",
                                            ),
                                        ),
                                        (
                                            "model",
                                            integers(
                                                field_type(neighbor_type, "model"),
                                                &target_fields(out.neighbor.model),
                                            ),
                                        ),
                                    ],
                                ),
                            ),
                            (
                                "place",
                                variant(field_type(output_type, "place"), place_tag),
                            ),
                        ],
                    )
                    .canonical_bytes()
                    .unwrap()
                });
                assert_eq!(evaluate(&programs, result, &encoded), expected);
            }
        }
    }
}
