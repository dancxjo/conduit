use super::{host, installed_std, BTreeMap, BaseImplementationId, PlanningOptions, RecordingTimer};
use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options};
use conduit_presentation::MAX_GRAPHICS_SCENE_BYTES;
use std::io::{self, Write};

struct LostGraphicsSurface;

impl Write for LostGraphicsSurface {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.starts_with(b"graphics scene commands=") {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "graphics surface lost",
            ))
        } else {
            Ok(bytes.len())
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const FORM: &str = r#"form canonical_gear_face {
 icon: presentation/icon(icon = "presentation", accessibility-name = "Patchbay")
 frame: presentation/frame(role = "panel", accessibility-name = "Gear Face")
 badge: presentation/badge(state = "ready", accessibility-name = "ready")
 rect: graphics/rect(style = "stroke")
 text: graphics/text(text = "ready")
 glyph: graphics/icon(icon = "presentation")
 sink: presentation/graphics
 icon.presented > frame.content
 frame.presented > badge.content
 badge.presented > rect.input
 rect.scene > text.input
 text.scene > glyph.input
 glyph.scene > sink.scene
}
"#;

#[test]
fn ordinary_presentation_back_lowers_through_multiple_graphics_kinds() {
    let mut host = host("graphics-conformance-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("graphics Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("graphics placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_GRAPHICS_SCENE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("graphics Form plans");
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 7);
    assert_eq!(fragment.connections.len(), 6);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let mut output = Vec::new();
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("graphics Form executes through production kernel");
    assert!(String::from_utf8(output)
        .unwrap()
        .contains("graphics scene commands=3\n"));
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );

    let error = host
        .run_fragment_to(fragment.clone(), &mut LostGraphicsSurface, &mut timer)
        .expect_err("lost graphics surface is not presentation success");
    assert!(error.contains("graphics surface lost"), "{error}");

    let mut unsupported = fragment.clone();
    let graphics = unsupported
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_std_catalog::GRAPHICS_TEXT_KIND)
        .unwrap();
    graphics.execution_profile_id =
        conduit_core::ExecutionProfileId::from("renderer/private-profile@1");
    let error = host
        .run_fragment_to(unsupported, &mut Vec::new(), &mut timer)
        .expect_err("unsupported graphics profile refuses before Play");
    assert!(
        error.contains("InvalidFragment"),
        "unexpected refusal: {error}"
    );
}
