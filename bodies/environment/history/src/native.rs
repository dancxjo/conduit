//! Normal finite GraphicsScene adapter. The caller owns checked Mask/Show admission.
use crate::graph::{ChartGeometry, HistoryGraph};
use alloc::format;
use conduit_face::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole as Paint, GraphicsScene, GraphicsShapeStyle,
    LayoutRect,
};
/// Correlation travels beside the native graphical payload, never in renderer state.
pub struct NativeHistoryShow<'a> {
    pub content_id: &'a str,
    pub revision: u64,
    pub scene: GraphicsScene,
}
impl<'a> HistoryGraph<'a> {
    pub fn native_show(&self) -> Result<NativeHistoryShow<'a>, GraphicsError> {
        Ok(NativeHistoryShow {
            content_id: self.content_id,
            revision: self.revision,
            scene: self.native_scene()?,
        })
    }
    pub fn native_scene(&self) -> Result<GraphicsScene, GraphicsError> {
        ChartGeometry {
            samples: &self.samples,
        }
        .native_scene(
            self.omitted,
            "RECORDED HISTORY",
            conduit_core::Unit::Celsius,
        )
    }
}
impl ChartGeometry<'_> {
    pub(crate) fn native_scene(
        &self,
        omitted: u64,
        provenance: &str,
        temperature_unit: conduit_core::Unit,
    ) -> Result<GraphicsScene, GraphicsError> {
        let mut scene = GraphicsScene::empty();
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width: 640,
            height: 360,
        };
        // Paper and ink both come from the admitted Look. An empty compositor
        // buffer alone cannot establish the selected background color.
        scene.push(GraphicsCommand::rect(
            clip,
            clip,
            Paint::Background,
            GraphicsShapeStyle::Fill,
        )?)?;
        if self.samples.is_empty() {
            scene.push(GraphicsCommand::text(
                LayoutRect {
                    x: 16,
                    y: 16,
                    width: 610,
                    height: 20,
                },
                clip,
                Paint::Foreground,
                "No recovered samples in this view",
            )?)?;
            return Ok(scene);
        }
        let (min, max) = self.temperature_range();
        if !matches!(
            temperature_unit,
            conduit_core::Unit::Celsius | conduit_core::Unit::Fahrenheit
        ) {
            return Err(GraphicsError::InvalidGeometry);
        }
        let display = |value: i64| {
            let quantity = conduit_core::Quantity::from_decimal(
                i128::from(value),
                -3,
                conduit_core::Unit::Celsius,
            )
            .map_err(|_| GraphicsError::InvalidGeometry)?;
            super::graph::quantity_milli(quantity, temperature_unit)
                .map_err(|_| GraphicsError::InvalidGeometry)
        };
        let unit_label = if temperature_unit == conduit_core::Unit::Fahrenheit {
            "°F"
        } else {
            "°C"
        };
        let first = self.samples.first().map_or(0, |s| s.boot_ms);
        let last = self.samples.last().map_or(0, |s| s.boot_ms);
        for (y, label) in [
            (
                4,
                format!(
                    "Temperature {}: top {}; bottom {}",
                    unit_label,
                    super::graph::milli_label(display(max)?),
                    super::graph::milli_label(display(min)?)
                ),
            ),
            (156, "Humidity %: top 100; bottom 0".into()),
            (
                306,
                format!("Boot ms: left {first}; right {last}; lines join adjacent samples"),
            ),
        ] {
            scene.push(GraphicsCommand::text(
                LayoutRect {
                    x: 16,
                    y,
                    width: 610,
                    height: 20,
                },
                clip,
                Paint::Foreground,
                &label,
            )?)?;
        }
        let failures = self.samples.iter().filter(|s| s.failure).count();
        let gaps = self
            .samples
            .iter()
            .filter(|s| s.gap_before.is_some())
            .count();
        let label = format!(
            "{}; outside {}; failures {}; gaps {}",
            provenance, omitted, failures, gaps
        );
        scene.push(GraphicsCommand::text(
            LayoutRect {
                x: 16,
                y: 334,
                width: 610,
                height: 20,
            },
            clip,
            Paint::Foreground,
            &label,
        )?)?;
        for humidity in [false, true] {
            let mut run = alloc::vec::Vec::with_capacity(super::graph::MAX_GRAPH_SAMPLES);
            let mut previous_sequence = None;
            for sample in &self.samples {
                let value = if humidity {
                    sample.humidity_milli_percent
                } else {
                    sample.temperature_milli_c
                };
                if sample.gap_before.is_some()
                    || previous_sequence
                        .is_some_and(|old: u64| old.checked_add(1) != Some(sample.sequence))
                    || value.is_none()
                {
                    self.flush_run(&mut scene, &run, clip)?;
                    run.clear();
                }
                if let Some(value) = value {
                    let point = conduit_face::GraphicsPoint::new(
                        self.x(sample) as i16,
                        self.y(value, humidity) as i16,
                    )?;
                    // Equal coordinates carry no additional visible segment.
                    if run.last() != Some(&point) {
                        run.push(point);
                    }
                }
                previous_sequence = Some(sample.sequence);
            }
            self.flush_run(&mut scene, &run, clip)?;
        }
        Ok(scene)
    }
    fn flush_run(
        &self,
        scene: &mut GraphicsScene,
        points: &[conduit_face::GraphicsPoint],
        clip: LayoutRect,
    ) -> Result<(), GraphicsError> {
        match points {
            [] => Ok(()),
            [point] => scene.push(GraphicsCommand::rect(
                LayoutRect {
                    x: *point.x(),
                    y: *point.y(),
                    width: 3,
                    height: 3,
                },
                clip,
                Paint::Foreground,
                GraphicsShapeStyle::Fill,
            )?),
            _ => scene.push(GraphicsCommand::polyline(
                conduit_face::GraphicsPolyline::new(points)?,
                clip,
                Paint::Foreground,
            )?),
        }
    }
}
