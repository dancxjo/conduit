//! Post-Play bench manifestation using the ordinary Face and seven-segment Mask.
//! This preparation phase allocates in the finite arena after the kernel drains.
use crate::usb_link::UsbLinkSession;
use conduit_core::Quantity;
use conduit_environment_face::{contribution, SensorStatus, SUBJECT};
use conduit_face::{Face, FaceBasis, FaceContributionBasis};
use conduit_seven_segment_mask::{
    render, PreparedEnvironmentFace, SegmentPatternForm, SevenSegmentLook,
    SevenSegmentSelection as Selection,
};
use core::fmt::Write;

pub async fn report(
    link: &mut UsbLinkSession,
    observation: Option<(Quantity, Quantity)>,
    status: SensorStatus,
    basis: &FaceBasis,
    fresh: bool,
) {
    let fragment = contribution(
        observation.map(|v| v.0),
        observation.map(|v| v.1),
        observation.is_some() && fresh,
        status,
        FaceContributionBasis {
            checked_plot_id: basis.checked_plot_id.clone().unwrap(),
            plan_id: basis.plan_id.clone().unwrap(),
            active_play_id: basis.active_play_id.clone().unwrap(),
            required_interaction_context: None,
        },
    )
    .unwrap();
    let face = Face::new(
        1,
        basis.clone(),
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
    )
    .unwrap();
    let view = PreparedEnvironmentFace::prepare(&face, SUBJECT).unwrap();
    for (selection, label) in [
        (Selection::Temperature, "temperature"),
        (Selection::Humidity, "humidity"),
    ] {
        let rendered = render(&view, selection, &SevenSegmentLook::fahrenheit());
        let digits = rendered
            .show
            .digits()
            .map(|d| SegmentPatternForm::encode(d)[0]);
        let points = rendered.show.decimal_points();
        let mut text = heapless::String::<8>::new();
        for (i, &bits) in digits.iter().enumerate() {
            text.push(glyph(bits)).unwrap();
            if points[i] {
                text.push('.').unwrap();
            }
        }
        let mut line = heapless::String::<768>::new();
        writeln!(line,
            "{{\"schema\":\"conduit.kernel-pulse-hil/v1\",\"phase\":\"show\",\"selection\":\"{label}\",\"look\":\"fahrenheit\",\"face\":\"{}\",\"revision\":{},\"text\":\"{text}\",\"digits\":[{},{},{},{}],\"decimal_points\":[{},{},{},{}],\"colon\":{}}}",
            rendered.face_content_id, rendered.face_revision,
            digits[0], digits[1], digits[2], digits[3],
            points[0], points[1], points[2], points[3], rendered.show.colon(),
        ).unwrap();
        let _ = link.write_text(&line).await;
    }
}
// Diagnostic transcription of the emitted Show, never a second formatter.
fn glyph(bits: u8) -> char {
    const NUMERALS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    if let Some(i) = NUMERALS.iter().position(|&v| v == bits) {
        return char::from(b'0' + i as u8);
    }
    match bits {
        0 => ' ',
        0x40 => '-',
        0x71 => 'F',
        0x39 => 'C',
        0x76 => 'H',
        _ => '?',
    }
}
