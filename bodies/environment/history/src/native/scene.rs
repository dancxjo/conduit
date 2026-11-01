use super::layout::{ChartLayout, NiceScale, decimal, elapsed};
use crate::graph::{ChartGeometry, GraphSample, MAX_GRAPH_SAMPLES, quantity_milli};
use alloc::{format, vec::Vec};
use conduit_core::{Quantity, Unit};
use conduit_face::{
    GraphicsCommand, GraphicsError, GraphicsPaintRole as Paint, GraphicsPoint, GraphicsPolyline,
    GraphicsScene, GraphicsShapeStyle, GraphicsTextRole as Role, LayoutRect,
};

fn converted(value: i64, unit: Unit) -> Result<i64, GraphicsError> {
    let quantity = Quantity::from_decimal(i128::from(value), -3, Unit::Celsius)
        .map_err(|_| GraphicsError::InvalidGeometry)?;
    quantity_milli(quantity, unit).map_err(|_| GraphicsError::InvalidGeometry)
}
fn label(
    scene: &mut GraphicsScene,
    clip: LayoutRect,
    bounds: LayoutRect,
    text: &str,
    role: Role,
    paint: Paint,
    measure: Option<&super::NativeHistoryTextMeasure<'_>>,
) -> Result<(), GraphicsError> {
    if let Some(measure) = measure {
        let measured = measure(text, role, bounds.width)?;
        if measured == 0 || measured > bounds.height {
            return Err(GraphicsError::InvalidGeometry);
        }
    }
    scene.push(GraphicsCommand::text(bounds, clip, paint, text)?.with_text_role(role)?)
}
fn breaks(sample: &GraphSample, previous: Option<u64>, value: Option<i64>) -> bool {
    sample.gap_before.is_some()
        || previous.is_some_and(|old| old.checked_add(1) != Some(sample.sequence))
        || value.is_none()
}
impl ChartGeometry<'_> {
    pub(crate) fn native_scene_at_size(
        &self,
        omitted: u64,
        provenance: &str,
        unit: Unit,
        width: u16,
        height: u16,
    ) -> Result<GraphicsScene, GraphicsError> {
        self.native_scene_at_size_measured(omitted, provenance, unit, (width, height), None)
    }
    pub(crate) fn native_scene_at_size_measured(
        &self,
        omitted: u64,
        provenance: &str,
        unit: Unit,
        size: (u16, u16),
        measure: Option<&super::NativeHistoryTextMeasure<'_>>,
    ) -> Result<GraphicsScene, GraphicsError> {
        let (width, height) = size;
        ChartLayout::new(width, height)?;
        if !matches!(unit, Unit::Celsius | Unit::Fahrenheit) {
            return Err(GraphicsError::InvalidGeometry);
        }
        let clip = LayoutRect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let mut scene = GraphicsScene::empty();
        scene.push(GraphicsCommand::rect(
            clip,
            clip,
            Paint::Background,
            GraphicsShapeStyle::Fill,
        )?)?;
        if self.samples.is_empty() {
            label(
                &mut scene,
                clip,
                LayoutRect {
                    x: 24,
                    y: 24,
                    width: width - 48,
                    height: if measure.is_some() { height - 48 } else { 40 },
                },
                "No retained readings",
                Role::Title,
                Paint::Foreground,
                measure,
            )?;
            return Ok(scene);
        }
        let mut temperatures = self
            .samples
            .iter()
            .filter_map(|s| s.temperature_milli_c)
            .map(|v| converted(v, unit));
        let (lo, hi) = if let Some(first) = temperatures.next() {
            let first = first?;
            temperatures.try_fold((first, first), |(lo, hi), v| {
                let v = v?;
                Ok::<_, GraphicsError>((lo.min(v), hi.max(v)))
            })?
        } else {
            (0, 0)
        };
        let temperature = NiceScale::from_values(lo, hi)?;
        let humidity = NiceScale {
            min: 0,
            max: 100_000,
        };
        let first = self.samples.first().unwrap().boot_ms;
        let last = self.samples.last().unwrap().boot_ms;
        let latest = self.samples.iter().rev().find(|s| !s.failure);
        let temp_latest = latest
            .and_then(|s| s.temperature_milli_c)
            .map(|v| converted(v, unit).map(decimal))
            .transpose()?
            .unwrap_or_else(|| "—".into());
        let humidity_latest = latest
            .and_then(|s| s.humidity_milli_percent)
            .map(decimal)
            .unwrap_or_else(|| "—".into());
        let unit_label = if unit == Unit::Fahrenheit {
            "°F"
        } else {
            "°C"
        };
        let headings = [
            if width < 480 {
                format!("Temp {temp_latest} {unit_label}")
            } else {
                format!("Temperature   {temp_latest} {unit_label}")
            },
            if width < 480 {
                format!("RH {humidity_latest} %")
            } else {
                format!("Humidity   {humidity_latest} %")
            },
        ];
        let role = if width < 480 {
            Role::Heading
        } else {
            Role::Title
        };
        let failures = self.samples.iter().filter(|s| s.failure).count();
        let gaps = self
            .samples
            .iter()
            .filter(|s| s.gap_before.is_some())
            .count();
        let footer = format!(
            "{provenance} · elapsed · {} points · {gaps} gaps · {failures} unavailable · +{omitted}",
            self.samples.len()
        );
        let mut heading_height = 40;
        let mut footer_height = 24;
        if let Some(measure) = measure {
            for heading in &headings {
                heading_height = heading_height.max(measure(
                    heading,
                    role,
                    width - if width >= 900 { 128 } else { 112 },
                )?);
            }
            footer_height = footer_height.max(measure(&footer, Role::Label, width - 32)?);
        }
        let layout = ChartLayout::with_text_heights(width, height, heading_height, footer_height)?;
        for (panel, title) in headings.iter().enumerate() {
            label(
                &mut scene,
                clip,
                LayoutRect {
                    x: layout.left,
                    y: layout.heading[panel],
                    width: (layout.right - layout.left) as u16,
                    height: heading_height,
                },
                title,
                if width < 480 {
                    Role::Heading
                } else {
                    Role::Title
                },
                Paint::Foreground,
                measure,
            )?;
        }
        let mut grid = Vec::with_capacity(8);
        for value in [temperature.max, temperature.midpoint(), temperature.min] {
            let y = layout.y(value, temperature, 0);
            label(
                &mut scene,
                clip,
                LayoutRect {
                    x: 16,
                    y: y - 10,
                    width: (layout.left - 28) as u16,
                    height: 24,
                },
                &if (temperature.max - temperature.min) / 2 >= 1000 && value % 1000 == 0 {
                    format!("{}", value / 1000)
                } else {
                    decimal(value)
                },
                Role::Label,
                Paint::Muted,
                measure,
            )?;
            grid.push(y);
        }
        for value in [100_000, 75_000, 50_000, 25_000, 0] {
            let y = layout.y(value, humidity, 1);
            label(
                &mut scene,
                clip,
                LayoutRect {
                    x: 16,
                    y: y - 10,
                    width: (layout.left - 28) as u16,
                    height: 24,
                },
                &format!("{}%", value / 1000),
                Role::Label,
                Paint::Muted,
                measure,
            )?;
            grid.push(y);
        }
        let times = [first, first + (last - first) / 2, last];
        for (i, time) in times.into_iter().enumerate() {
            if width < 480 && i == 1 {
                continue;
            }
            let x = match i {
                0 if width < 480 => 16,
                0 => layout.left,
                1 => (layout.left + layout.right) / 2 - 60,
                _ => layout.right - 120,
            };
            label(
                &mut scene,
                clip,
                LayoutRect {
                    x,
                    y: height as i16 - footer_height as i16 - 38,
                    width: 120,
                    height: 24,
                },
                &elapsed(time),
                Role::Label,
                Paint::Muted,
                measure,
            )?;
        }
        label(
            &mut scene,
            clip,
            LayoutRect {
                x: 16,
                y: height as i16 - footer_height as i16 - 2,
                width: width - 32,
                height: footer_height,
            },
            &footer,
            Role::Label,
            Paint::Muted,
            measure,
        )?;
        // Fixed decoration:15 commands. At most32 series runs. Only actual
        // remaining slots become grids; never enlarge the48-command grammar.
        let data_commands = self.run_count(false) + self.run_count(true);
        let remaining = 48usize
            .checked_sub(scene.commands().len() + data_commands)
            .ok_or(GraphicsError::TooManyCommands)?;
        for y in grid.into_iter().take(remaining) {
            scene.push(GraphicsCommand::rect(
                LayoutRect {
                    x: layout.left,
                    y,
                    width: (layout.right - layout.left) as u16,
                    height: 1,
                },
                clip,
                Paint::Border,
                GraphicsShapeStyle::Fill,
            )?)?;
        }
        for (panel, scale) in [temperature, humidity].into_iter().enumerate() {
            let mut run = Vec::with_capacity(MAX_GRAPH_SAMPLES);
            let mut previous = None;
            for sample in self.samples {
                let value = if panel == 1 {
                    sample.humidity_milli_percent
                } else {
                    sample.temperature_milli_c
                };
                if breaks(sample, previous, value) {
                    flush(&mut scene, &run, clip)?;
                    run.clear();
                }
                if let Some(value) = value {
                    let value = if panel == 0 {
                        converted(value, unit)?
                    } else {
                        value
                    };
                    let point = GraphicsPoint::new(
                        layout.x(sample.boot_ms, first, last),
                        layout.y(value, scale, panel),
                    )
                    .map_err(|_| GraphicsError::InvalidGeometry)?;
                    if run.last() != Some(&point) {
                        run.push(point);
                    }
                }
                previous = Some(sample.sequence);
            }
            flush(&mut scene, &run, clip)?;
        }
        Ok(scene)
    }
    fn run_count(&self, humidity: bool) -> usize {
        let (mut count, mut active, mut previous) = (0, false, None);
        for sample in self.samples {
            let value = if humidity {
                sample.humidity_milli_percent
            } else {
                sample.temperature_milli_c
            };
            if breaks(sample, previous, value) {
                active = false;
            }
            if value.is_some() && !active {
                count += 1;
                active = true;
            }
            previous = Some(sample.sequence);
        }
        count
    }
}
fn flush(
    scene: &mut GraphicsScene,
    points: &[GraphicsPoint],
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
            Paint::Accent,
            GraphicsShapeStyle::Fill,
        )?),
        _ => scene.push(GraphicsCommand::polyline(
            GraphicsPolyline::new(points)?,
            clip,
            Paint::Accent,
        )?),
    }
}

#[cfg(test)]
mod selected_measurement_tests {
    use super::*;
    #[test]
    fn unmeasured_empty_graph_retains_original_label_geometry() {
        let geometry = ChartGeometry { samples: &[] };
        let scene = geometry
            .native_scene_at_size(0, "retained", Unit::Fahrenheit, 640, 360)
            .unwrap();
        let text = scene
            .commands()
            .iter()
            .find(|c| c.payload() == "No retained readings")
            .unwrap();
        assert_eq!(text.bounds.height, 40);
    }
    #[test]
    fn oversized_selected_label_refuses_without_scene() {
        let geometry = ChartGeometry { samples: &[] };
        let measure = |_: &str, _: Role, _: u16| Ok(u16::MAX);
        assert!(
            geometry
                .native_scene_at_size_measured(
                    0,
                    "retained",
                    Unit::Fahrenheit,
                    (640, 360),
                    Some(&measure)
                )
                .is_err()
        );
    }
}
