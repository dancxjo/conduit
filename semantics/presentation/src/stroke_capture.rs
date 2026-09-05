//! Finite ordered point capture independent of input devices and renderers.

use alloc::{string::String, vec::Vec};
use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};

use crate::{path2_value, point2_type, GeometryRefusal, MAXIMUM_GEOMETRY_PATH_POINTS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StrokeCaptureRefusal {
    InvalidCapacity,
    MalformedPoint,
    FrameMismatch { expected: String, actual: String },
    Pressure,
    PartialStroke,
    Geometry(GeometryRefusal),
}

#[derive(Debug, Clone)]
pub struct BoundedStrokeCapture {
    maximum_points: u16,
    minimum_points: u16,
    frame: Option<String>,
    points: Vec<StructuredInfoValue>,
}

impl BoundedStrokeCapture {
    pub fn new(maximum_points: u16, minimum_points: u16) -> Result<Self, StrokeCaptureRefusal> {
        if maximum_points == 0
            || maximum_points > MAXIMUM_GEOMETRY_PATH_POINTS
            || minimum_points == 0
            || minimum_points > maximum_points
        {
            return Err(StrokeCaptureRefusal::InvalidCapacity);
        }
        Ok(Self {
            maximum_points,
            minimum_points,
            frame: None,
            points: Vec::with_capacity(usize::from(maximum_points)),
        })
    }

    pub fn push(&mut self, point: StructuredInfoValue) -> Result<(), StrokeCaptureRefusal> {
        if point.value_type() != &point2_type() {
            return Err(StrokeCaptureRefusal::MalformedPoint);
        }
        if self.points.len() == usize::from(self.maximum_points) {
            return Err(StrokeCaptureRefusal::Pressure);
        }
        let frame = point_frame(&point)?;
        if let Some(expected) = &self.frame {
            if expected != &frame {
                return Err(StrokeCaptureRefusal::FrameMismatch {
                    expected: expected.clone(),
                    actual: frame,
                });
            }
        } else {
            self.frame = Some(frame);
        }
        self.points.push(point);
        Ok(())
    }

    pub fn finish(self) -> Result<StructuredInfoValue, StrokeCaptureRefusal> {
        if self.points.len() < usize::from(self.minimum_points) {
            return Err(StrokeCaptureRefusal::PartialStroke);
        }
        path2_value(self.points).map_err(StrokeCaptureRefusal::Geometry)
    }

    pub fn point_count(&self) -> usize {
        self.points.len()
    }
}

fn point_frame(point: &StructuredInfoValue) -> Result<String, StrokeCaptureRefusal> {
    let StructuredInfoValueShape::Record(fields) = point.shape() else {
        return Err(StrokeCaptureRefusal::MalformedPoint);
    };
    let frame = fields
        .iter()
        .find(|field| field.name() == "frame")
        .map(StructuredFieldValue::value)
        .ok_or(StrokeCaptureRefusal::MalformedPoint)?;
    let StructuredInfoValueShape::Leaf(bytes) = frame.shape() else {
        return Err(StrokeCaptureRefusal::MalformedPoint);
    };
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| StrokeCaptureRefusal::MalformedPoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{Quantity, QuantityUnit};

    fn point(frame: &str, x: i64) -> StructuredInfoValue {
        crate::point2_value(
            frame,
            Quantity::new(x, QuantityUnit::Millimeter),
            Quantity::new(0, QuantityUnit::Millimeter),
        )
        .unwrap()
    }

    #[test]
    fn ordered_points_finish_as_one_bounded_path() {
        let mut capture = BoundedStrokeCapture::new(4, 4).unwrap();
        capture.push(point("drawing/local", 0)).unwrap();
        capture.push(point("drawing/local", 5)).unwrap();
        capture.push(point("drawing/local", 8)).unwrap();
        capture.push(point("drawing/local", 13)).unwrap();
        let path = capture.finish().unwrap();
        assert_eq!(path.value_type(), &crate::path2_type(4).unwrap());
    }

    #[test]
    fn partial_pressure_and_frame_mismatch_are_distinct() {
        let mut partial = BoundedStrokeCapture::new(2, 2).unwrap();
        partial.push(point("drawing/local", 0)).unwrap();
        assert_eq!(partial.finish(), Err(StrokeCaptureRefusal::PartialStroke));

        let mut full = BoundedStrokeCapture::new(1, 1).unwrap();
        full.push(point("drawing/local", 0)).unwrap();
        assert_eq!(
            full.push(point("drawing/local", 1)),
            Err(StrokeCaptureRefusal::Pressure)
        );

        let mut mixed = BoundedStrokeCapture::new(2, 1).unwrap();
        mixed.push(point("drawing/local", 0)).unwrap();
        assert!(matches!(
            mixed.push(point("drawing/remote", 1)),
            Err(StrokeCaptureRefusal::FrameMismatch { .. })
        ));
    }
}
