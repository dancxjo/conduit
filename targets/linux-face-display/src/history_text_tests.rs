//! Selected Look geometry for recorded/retained charts, not custody or live-feed proof.
use conduit_core::{Quantity, Unit};
use conduit_environment_history::RetainedEvidenceGraph;
use conduit_environment_runtime::{
    observed_record::ObservedRecord, recording::RecordingDisposition,
};
use conduit_face::{GraphicsCommandKind as Kind, GraphicsScene, GraphicsTextRole};

fn graph() -> RetainedEvidenceGraph {
    let mut bytes = vec![];
    for i in 0..16u64 {
        bytes.extend(
            ObservedRecord {
                sequence: i * 2 + 1,
                observed_at_boot_ms: i * 1000,
                disposition: if i % 4 == 3 {
                    RecordingDisposition::Checksum
                } else {
                    RecordingDisposition::Available
                },
                quantities: (i % 4 != 3).then_some((
                    Quantity::new(20, Unit::Celsius),
                    Quantity::new(54, Unit::Percent),
                )),
            }
            .encode()
            .unwrap(),
        );
    }
    RetainedEvidenceGraph::prepare(&bytes, 31, u32::MAX).unwrap()
}
fn footer(scene: &GraphicsScene) -> &conduit_face::GraphicsCommand {
    scene
        .commands()
        .iter()
        .find(|c| c.payload().contains(" · elapsed · "))
        .unwrap()
}
#[test]
fn every_look_measures_complete_chart_text_at_actual_and_minimum_column_widths() {
    let graph = graph();
    let original = graph.samples().to_vec();
    for look in [
        conduit_face::phosphor_look().unwrap(),
        conduit_face::editorial_look().unwrap(),
        conduit_face::engineering_look().unwrap(),
        conduit_face::navy_look(
            conduit_face::VisualDensity::Compact,
            conduit_face::VisualTypography::Editorial,
        )
        .unwrap(),
        conduit_face::field_instrument_day_look().unwrap(),
        conduit_face::field_instrument_night_look().unwrap(),
    ] {
        let typography = crate::raster_target::selected_typography(Some(&look)).unwrap();
        for width in [320, 566, 588, 692, 1024] {
            for height in [360, 476] {
                let scene = graph
                    .native_scene_in_unit_at_size_with_window_measured(
                        Unit::Fahrenheit,
                        width,
                        height,
                        None,
                        &|text, role, width| {
                            crate::raster_target::measure_chart_text(typography, text, role, width)
                        },
                    )
                    .unwrap();
                assert!(scene.commands().len() <= 48);
                for command in scene.commands().iter().filter(|c| c.kind == Kind::Text) {
                    let measured = crate::raster_target::measure_chart_text(
                        typography,
                        command.payload(),
                        command.text_role(),
                        command.bounds.width,
                    )
                    .unwrap();
                    assert!(
                        measured <= command.bounds.height,
                        "{} at {width}x{height}",
                        command.payload()
                    );
                    assert!(
                        i32::from(command.bounds.y) + i32::from(command.bounds.height)
                            <= i32::from(height)
                    );
                    assert!(command.payload().len() <= 192);
                }
                let footer = footer(&scene);
                assert_eq!(
                    footer.payload(),
                    "RETAINED HISTORY · elapsed · 16 points · 15 gaps · 4 unavailable · +4294967295"
                );
                if width <= 588 {
                    assert!(footer.bounds.height > 24);
                }
                // Footer preparation moves the graph above it rather than covering observations.
                for command in scene.commands().iter().filter(|c| {
                    c.paint == conduit_face::GraphicsPaintRole::Accent && c.kind != Kind::Text
                }) {
                    assert!(command.bounds.y < footer.bounds.y);
                }
                assert_eq!(graph.samples(), original);
            }
        }
    }
}
#[test]
fn impossible_or_failed_chart_text_measurement_refuses_without_changing_samples() {
    let graph = graph();
    let original = graph.samples().to_vec();
    for height in [0, u16::MAX] {
        assert!(
            graph
                .native_scene_in_unit_at_size_with_window_measured(
                    Unit::Celsius,
                    320,
                    360,
                    None,
                    &|_, _, _| Ok(height)
                )
                .is_err()
        );
    }
    assert!(
        graph
            .native_scene_in_unit_at_size_with_window_measured(
                Unit::Celsius,
                588,
                476,
                None,
                &|_, role, _| if role == GraphicsTextRole::Label {
                    Err(conduit_face::GraphicsError::InvalidGeometry)
                } else {
                    Ok(40)
                }
            )
            .is_err()
    );
    assert_eq!(graph.samples(), original);
}

#[test]
#[ignore = "requires explicitly selected private verified historical evidence and output directory"]
fn actual_retained_evidence_uses_measured_field_looks_without_touching_the_display() {
    use std::io::Write;
    let path =
        std::env::var_os("CONDUIT_PRIVATE_HISTORY_PREVIEW").expect("private actual evidence");
    let output = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PRIVATE_HISTORY_PREVIEW_DIRECTORY").expect("private output"),
    );
    std::fs::create_dir_all(&output).unwrap();
    let original = std::fs::read(&path).unwrap();
    for (name, look) in [
        ("day", conduit_face::field_instrument_day_look().unwrap()),
        (
            "night",
            conduit_face::field_instrument_night_look().unwrap(),
        ),
    ] {
        let typography = crate::raster_target::selected_typography(Some(&look)).unwrap();
        for width in [320, 588] {
            let mut input =
                crate::history_input::HistoryInput::new_at_size(Some(Unit::Fahrenheit), width, 476);
            input
                .configure(Unit::Fahrenheit, width, 476, None, Some(typography))
                .unwrap();
            assert!(input.refresh(std::path::Path::new(&path)).unwrap());
            let scene = input.scene().unwrap();
            assert!(footer(scene).bounds.height > 24);
            for command in scene.commands().iter().filter(|c| c.kind == Kind::Text) {
                assert!(
                    crate::raster_target::measure_chart_text(
                        typography,
                        command.payload(),
                        command.text_role(),
                        command.bounds.width
                    )
                    .unwrap()
                        <= command.bounds.height
                );
            }
            let mut pixels = vec![0; usize::from(width) * 476];
            crate::raster_target::panel(scene, width, 476, &mut pixels, &look).unwrap();
            let mut file =
                std::fs::File::create(output.join(format!("history-{name}-{width}.ppm"))).unwrap();
            write!(file, "P6\n{width} 476\n255\n").unwrap();
            let rgb: Vec<u8> = pixels
                .iter()
                .flat_map(|p| [(*p >> 16) as u8, (*p >> 8) as u8, *p as u8])
                .collect();
            file.write_all(&rgb).unwrap();
        }
    }
    assert_eq!(std::fs::read(path).unwrap(), original);
}
