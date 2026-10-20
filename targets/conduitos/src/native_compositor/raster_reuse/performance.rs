//! Descriptive, finite before/after raster work on the same retained pixel target.
use super::super::surface_buffer::SurfaceBuffer;
use super::{canonical, raster};
use crate::display::{DisplayError, DisplayReceipt};
use conduit_face::{
    GraphicsCommand, GraphicsPaintRole as Paint, GraphicsScene, GraphicsShapeStyle, LayoutRect,
};
use std::{hint::black_box, time::Instant};

fn rect(x: i16, y: i16, width: u16, height: u16) -> LayoutRect {
    LayoutRect {
        x,
        y,
        width,
        height,
    }
}
fn scene(case: &str, alternate: bool) -> GraphicsScene {
    let extent = rect(0, 0, 640, 480);
    let mut result = GraphicsScene::empty();
    let background = if case == "full" && alternate {
        Paint::Muted
    } else {
        Paint::Background
    };
    result
        .push(GraphicsCommand::rect(extent, extent, background, GraphicsShapeStyle::Fill).unwrap())
        .unwrap();
    let content = if case == "scroll" && alternate {
        rect(24, 64, 360, 256)
    } else {
        rect(24, 80, 360, 256)
    };
    result.push(GraphicsCommand::text(content, extent, Paint::Foreground,
        "Prepared content remains part of the exact Show.\nRepeated text supplies a bounded raster workload.\nThe background behind every glyph is restored before replay.").unwrap()).unwrap();
    let hover = if case == "hover" && alternate {
        Paint::Warning
    } else {
        Paint::Accent
    };
    result
        .push(
            GraphicsCommand::rect(
                rect(480, 320, 32, 24),
                extent,
                hover,
                GraphicsShapeStyle::RoundedFill,
            )
            .unwrap(),
        )
        .unwrap();
    let text = if case == "typing" && alternate {
        "Wi"
    } else {
        "i"
    };
    result
        .push(
            GraphicsCommand::text(rect(24, 400, 64, 24), extent, Paint::Foreground, text).unwrap(),
        )
        .unwrap();
    let tiny = if case == "tiny" && alternate {
        Paint::Success
    } else {
        Paint::Danger
    };
    result
        .push(
            GraphicsCommand::rect(rect(600, 440, 2, 2), extent, tiny, GraphicsShapeStyle::Fill)
                .unwrap(),
        )
        .unwrap();
    result
}
fn render(
    buffer: &mut SurfaceBuffer,
    scene: &GraphicsScene,
) -> Result<DisplayReceipt, DisplayError> {
    crate::display::typography::render_scene(buffer, scene, |_, command| command.text_role().into())
}

#[test]
#[ignore = "manual optimized retained raster observation"]
fn retained_raster_performance() {
    let frames = 64;
    for case in ["full", "tiny", "scroll", "hover", "typing"] {
        let scenes = [scene(case, false), scene(case, true)];
        assert!(scenes.iter().all(canonical));
        let mut full = SurfaceBuffer::new(640, 480).unwrap();
        let mut retained = SurfaceBuffer::new(640, 480).unwrap();
        // Prime typography and prove every alternating revision before timing.
        let mut prior = scenes[0];
        raster(&mut retained, &prior, None, render).unwrap();
        for next in &scenes {
            raster(&mut full, next, None, render).unwrap();
            raster(&mut retained, next, Some(&prior), render).unwrap();
            assert_eq!(full.pixels, retained.pixels, "case={case}");
            prior = *next;
        }
        for repeat in 0usize..5 {
            let mut elapsed = [0; 2];
            let mut pixels = [0u64; 2];
            let mut commands = [0u64; 2];
            for implementation in if repeat.is_multiple_of(2) {
                [0, 1]
            } else {
                [1, 0]
            } {
                let buffer = if implementation == 0 {
                    &mut full
                } else {
                    &mut retained
                };
                // Each trial starts on the same previous scene, outside its timer.
                raster(buffer, &scenes[0], None, render).unwrap();
                let mut previous = scenes[0];
                let start = Instant::now();
                for frame in 0..frames {
                    let next = black_box(&scenes[(frame + 1) % 2]);
                    // Include eligibility checking and the exact cache copy in
                    // the optimized trial; the old path has neither cost.
                    let eligible = implementation == 1 && canonical(next);
                    let receipt = raster(
                        black_box(&mut *buffer),
                        next,
                        eligible.then_some(&previous),
                        render,
                    )
                    .unwrap();
                    if implementation == 1 {
                        previous = black_box(*next);
                    }
                    pixels[implementation] += u64::from(receipt.pixels_written);
                    commands[implementation] += u64::from(receipt.commands);
                    black_box(receipt);
                }
                elapsed[implementation] = start.elapsed().as_nanos();
            }
            assert_eq!(full.pixels, retained.pixels, "case={case}, repeat={repeat}");
            println!(
                "retained-raster case={case} repeat={repeat} frames={frames} reference_ns={} retained_ns={} reference_pixels={} retained_pixels={} reference_commands={} retained_commands={}",
                elapsed[0], elapsed[1], pixels[0], pixels[1], commands[0], commands[1]
            );
        }
    }
}
