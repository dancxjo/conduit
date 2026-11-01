//! Finite comparative geometry over one collector clock, shared by Masks.
use crate::{
    graph::quantity_milli,
    native::layout::{decimal, elapsed, NiceScale},
    source_series::*,
};
use alloc::{format, vec::Vec};
use conduit_core::Unit;
use conduit_environment_measurement::{source_observation::SourceOutcome, ReadingField};
use conduit_face::{
    GraphicsCommand as Command, GraphicsError as Error, GraphicsPaintRole as Paint,
    GraphicsPoint as Point, GraphicsPolyline, GraphicsScene, GraphicsShapeStyle as Shape,
    GraphicsTextRole as Text, LayoutRect as Rect,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceSeriesStyle {
    pub paint: Paint,
    pub dashed: bool,
}
/// The caller selects expression from a Look/local preference. It does not
/// choose a measurement, identity, clock mapping or availability state.
pub type SourceSeriesStyleResolver = fn(&PreparedSourceSeries) -> SourceSeriesStyle;
fn text(
    scene: &mut GraphicsScene,
    clip: Rect,
    bounds: Rect,
    value: &str,
    paint: Paint,
) -> Result<(), Error> {
    scene.push(Command::text(bounds, clip, paint, value)?.with_text_role(Text::Label)?)
}
fn coordinate(point: &SourceSeriesPoint, panel: usize, unit: Unit) -> Result<Option<i64>, Error> {
    let SourceOutcome::Available(sample) = point.observation.outcome else {
        return Ok(None);
    };
    let quantity = if panel == 0 {
        Some(sample.reading.temperature())
    } else {
        match sample.reading.relative_humidity() {
            ReadingField::Available(q) => Some(q),
            ReadingField::Unsupported => None,
        }
    };
    quantity
        .map(|q| {
            quantity_milli(q, if panel == 0 { unit } else { Unit::Percent })
                .map_err(|_| Error::InvalidGeometry)
        })
        .transpose()
}
fn flush(
    scene: &mut GraphicsScene,
    run: &[Point],
    clip: Rect,
    style: SourceSeriesStyle,
) -> Result<(), Error> {
    if style.dashed && run.len() > 1 {
        for pair in run.chunks(2) {
            flush(
                scene,
                pair,
                clip,
                SourceSeriesStyle {
                    dashed: false,
                    ..style
                },
            )?;
        }
        return Ok(());
    }
    match run {
        [] => Ok(()),
        [point] => scene.push(Command::rect(
            Rect {
                x: *point.x(),
                y: *point.y(),
                width: 3,
                height: 3,
            },
            clip,
            style.paint,
            Shape::Fill,
        )?),
        _ => scene.push(Command::polyline(
            GraphicsPolyline::new(run)?,
            clip,
            style.paint,
        )?),
    }
}
impl SourceSeriesGraph {
    /// All series share both quantity axes and collector-observed time. The
    /// complete scene must fit existing48-command admission or refuses intact.
    pub fn native_scene_at_size(
        &self,
        unit: Unit,
        width: u16,
        height: u16,
        window_ms: Option<u64>,
        style: impl Fn(&PreparedSourceSeries) -> SourceSeriesStyle,
    ) -> Result<GraphicsScene, Error> {
        if !matches!(unit, Unit::Celsius | Unit::Fahrenheit)
            || width < 320
            || height < 360
            || width > i16::MAX as u16
            || height > i16::MAX as u16
        {
            return Err(Error::InvalidGeometry);
        }
        let clip = Rect {
            x: 0,
            y: 0,
            width,
            height,
        };
        let styles: Vec<_> = self.series.iter().map(style).collect();
        let mut scene = GraphicsScene::empty();
        scene.push(Command::rect(clip, clip, Paint::Background, Shape::Fill)?)?;
        let cutoff = self.observed_through_boot_ms;
        let first = self
            .series
            .iter()
            .flat_map(|s| s.points.iter())
            .map(|p| p.observation.observed_at_host_boot_ms)
            .min()
            .unwrap_or(cutoff);
        let first = window_ms.map_or(first, |w| first.max(cutoff.saturating_sub(w)));
        let left = 72i16;
        let right = width as i16 - 16;
        let panel_height = (height as i16 - 132) / 2;
        let tops = [84i16, 84 + panel_height];
        let bottoms = [84 + panel_height - 38, 84 + 2 * panel_height - 38];
        let visible = |p: &&SourceSeriesPoint| p.observation.observed_at_host_boot_ms >= first;
        let mut lo = None;
        let mut hi = None;
        for s in &self.series {
            for p in s.points.iter().filter(visible) {
                if let Some(v) = coordinate(p, 0, unit)? {
                    lo = Some(lo.map_or(v, |x: i64| x.min(v)));
                    hi = Some(hi.map_or(v, |x: i64| x.max(v)));
                }
            }
        }
        let scales = [
            NiceScale::from_values(lo.unwrap_or(0), hi.unwrap_or(0))?,
            NiceScale {
                min: 0,
                max: 100000,
            },
        ];
        let cell = (width - 16) / MAX_SOURCE_SERIES as u16;
        for (i, s) in self.series.iter().enumerate() {
            let paint = styles[i].paint;
            let x = 8 + i as i16 * cell as i16;
            text(
                &mut scene,
                clip,
                Rect {
                    x,
                    y: 2,
                    width: cell,
                    height: 24,
                },
                &s.name,
                paint,
            )?;
            let state = match s.state_at(cutoff).map_err(|_| Error::InvalidGeometry)? {
                SourceSeriesState::Empty => "empty",
                SourceSeriesState::Unavailable(_) => "failed",
                SourceSeriesState::Stale { .. } => "stale",
                SourceSeriesState::Available { .. } => "ok",
            };
            text(
                &mut scene,
                clip,
                Rect {
                    x,
                    y: 26,
                    width: cell,
                    height: 24,
                },
                state,
                paint,
            )?;
        }
        for (panel, scale) in scales.into_iter().enumerate() {
            let title = if panel == 0 {
                if unit == Unit::Fahrenheit {
                    "Temperature (°F)"
                } else {
                    "Temperature (°C)"
                }
            } else {
                "Humidity (%)"
            };
            text(
                &mut scene,
                clip,
                Rect {
                    x: left,
                    y: tops[panel] - 26,
                    width: width - 88,
                    height: 24,
                },
                title,
                Paint::Foreground,
            )?;
            for v in [scale.max, scale.midpoint(), scale.min] {
                let y = bottoms[panel]
                    - ((v - scale.min) * i128::from(bottoms[panel] - tops[panel])
                        / (scale.max - scale.min)) as i16;
                text(
                    &mut scene,
                    clip,
                    Rect {
                        x: 4,
                        y: y - 10,
                        width: 64,
                        height: 24,
                    },
                    &decimal(v),
                    Paint::Muted,
                )?;
                // Axes are required; optional decorative grids are omitted so
                // finite commands remain available for actual disjoint runs.
            }
            scene.push(Command::rect(
                Rect {
                    x: left,
                    y: bottoms[panel],
                    width: (right - left) as u16,
                    height: 1,
                },
                clip,
                Paint::Border,
                Shape::Fill,
            )?)?;
            for (series_index, s) in self.series.iter().enumerate() {
                let mut run = Vec::with_capacity(MAX_SOURCE_SERIES_SAMPLES);
                let mut previous = None;
                for p in s.points.iter().filter(visible) {
                    let value = coordinate(p, panel, unit)?;
                    if value.is_none()
                        || previous.is_some_and(|n: u64| n.checked_add(1) != Some(p.sequence))
                    {
                        flush(&mut scene, &run, clip, styles[series_index])?;
                        run.clear();
                    }
                    if let Some(value) = value {
                        let at = p.observation.observed_at_host_boot_ms;
                        let x = if cutoff == first {
                            right
                        } else {
                            left + (u128::from(at.saturating_sub(first)) * (right - left) as u128
                                / u128::from(cutoff - first))
                                as i16
                        };
                        let y = bottoms[panel]
                            - ((i128::from(value).clamp(scale.min, scale.max) - scale.min)
                                * i128::from(bottoms[panel] - tops[panel])
                                / (scale.max - scale.min)) as i16;
                        let point = Point::new(x, y).map_err(|_| Error::InvalidGeometry)?;
                        if run.last() != Some(&point) {
                            run.push(point);
                        }
                    }
                    previous = Some(p.sequence);
                }
                flush(&mut scene, &run, clip, styles[series_index])?;
            }
        }
        text(
            &mut scene,
            clip,
            Rect {
                x: left,
                y: height as i16 - 62,
                width: width - 88,
                height: 24,
            },
            &format!("{} — {}", elapsed(first), elapsed(cutoff)),
            Paint::Muted,
        )?;
        text(
            &mut scene,
            clip,
            Rect {
                x: 8,
                y: height as i16 - 36,
                width: width - 16,
                height: 24,
            },
            "Collector observed · Boot time",
            Paint::Muted,
        )?;
        Ok(scene)
    }
}
