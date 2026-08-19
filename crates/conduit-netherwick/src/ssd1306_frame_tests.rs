use super::*;
use conduit_core::{BootId, HostId, OfferGeneration, SignId};
use conduit_presentation::{
    PresentationBasis, PresentationRole, PresentationSubject, PresentationText,
};

pub(crate) fn presentation(revision: u64) -> Presentation {
    Presentation::new(
        revision,
        PresentationBasis {
            seed_id: None,
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![SignId::from(format!("sign/{revision}"))],
        },
        vec![PresentationSubject {
            identity: "host/pico".into(),
            role: PresentationRole::Host,
            label: "Netherwick Host".into(),
            accessibility_name: "Netherwick Pico W Host".into(),
        }],
        Vec::new(),
        Vec::new(),
        vec![
            PresentationText {
                subject: "host/pico".into(),
                text: "Battery 88%".into(),
            },
            PresentationText {
                subject: "host/pico".into(),
                text: "Safety clear".into(),
            },
        ],
    )
    .unwrap()
}

#[test]
fn portable_text_projects_to_two_bounded_lines_and_fixed_frame() {
    let frame = project_ssd1306_frame(&presentation(1)).unwrap();
    assert_eq!(frame.lines[0].as_bytes(), b"BATTERY 88%");
    assert_eq!(frame.lines[1].as_bytes(), b"SAFETY CLEAR");
    assert_eq!(frame.framebuffer.len(), conduit_ssd1306::FRAMEBUFFER_BYTES);
    assert!(frame.framebuffer.iter().any(|byte| *byte != 0));
}

#[test]
fn projection_contains_no_host_base_or_private_status_input() {
    let presentation = presentation(1);
    let frame = project_ssd1306_frame(&presentation).unwrap();
    let rendered = frame
        .lines
        .iter()
        .flat_map(OledLine::as_bytes)
        .copied()
        .collect::<Vec<_>>();
    assert!(!String::from_utf8(rendered).unwrap().contains("I2C"));
    let _unrelated_realization_truth = (
        HostId::from("host/other"),
        BootId::from("boot/other"),
        OfferGeneration(99),
    );
    assert_eq!(project_ssd1306_frame(&presentation).unwrap(), frame);
}
