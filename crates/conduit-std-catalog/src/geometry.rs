//! Host-neutral finite geometry expressed entirely as ordinary structured Info.

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, Quantity, QuantityConversionRefusal, QuantityDimension, QuantityUnit,
    StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredInfoValueShape, MAXIMUM_STRUCTURED_COLLECTION_ITEMS,
};

pub const POINT2_TYPE: &str = "Point2";
pub const POINT3_TYPE: &str = "Point3";
pub const VECTOR2_TYPE: &str = "Vector2";
pub const VECTOR3_TYPE: &str = "Vector3";
pub const EXTENT2_TYPE: &str = "Extent2";
pub const RECT2_TYPE: &str = "Rect2";
pub const TRANSFORM2_TYPE: &str = "Transform2";
pub const PATH2_FOUR_TYPE: &str = "Path2Four";
pub const ROBOTICS_POSE2_TYPE: &str = "RoboticsPose2";
pub const IMAGE_REGION2_TYPE: &str = "ImageRegion2";
pub const MAXIMUM_GEOMETRY_PATH_POINTS: u16 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryRefusal {
    MalformedInfo,
    FrameMismatch { expected: String, actual: String },
    IncompatibleUnit,
    InexactUnitConversion,
    Overflow,
    TooManyPoints,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for GeometryRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text@1")).expect("text is finite")
}

fn quantity_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID)).expect("quantity is finite")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed geometry field")
}

fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).expect("reviewed geometry field")
}

pub fn point2_type() -> StructuredInfoType {
    coordinate_type("geometry/point2@1", &["x", "y"])
}

pub fn point3_type() -> StructuredInfoType {
    coordinate_type("geometry/point3@1", &["x", "y", "z"])
}

pub fn vector2_type() -> StructuredInfoType {
    coordinate_type("geometry/vector2@1", &["x", "y"])
}

pub fn vector3_type() -> StructuredInfoType {
    coordinate_type("geometry/vector3@1", &["x", "y", "z"])
}

fn coordinate_type(schema: &str, axes: &[&str]) -> StructuredInfoType {
    let mut fields = vec![field("frame", text_type())];
    fields.extend(axes.iter().map(|axis| field(axis, quantity_type())));
    StructuredInfoType::record(kind_id(schema), fields).expect("reviewed coordinate schema")
}

pub fn extent2_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("geometry/extent2@1"),
        vec![
            field("height", quantity_type()),
            field("width", quantity_type()),
        ],
    )
    .expect("reviewed extent schema")
}

pub fn rect2_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("geometry/rect2@1"),
        vec![
            field("extent", extent2_type()),
            field("origin", point2_type()),
        ],
    )
    .expect("reviewed rectangle schema")
}

/// A deliberately small transform: exact translation between two named frames.
/// Rotation and arbitrary matrices are not smuggled into this first contract.
pub fn transform2_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("geometry/translation-transform2@1"),
        vec![
            field("from_frame", text_type()),
            field("offset_x", quantity_type()),
            field("offset_y", quantity_type()),
            field("to_frame", text_type()),
        ],
    )
    .expect("reviewed transform schema")
}

pub fn path2_type(point_count: u16) -> Result<StructuredInfoType, GeometryRefusal> {
    if point_count == 0
        || point_count > MAXIMUM_GEOMETRY_PATH_POINTS
        || usize::from(point_count) > MAXIMUM_STRUCTURED_COLLECTION_ITEMS
    {
        return Err(GeometryRefusal::TooManyPoints);
    }
    let points = StructuredInfoType::collection(point2_type(), Some(point_count))?;
    Ok(StructuredInfoType::record(
        kind_id("geometry/path2@1"),
        vec![field("points", points)],
    )?)
}

pub fn robotics_pose2_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("robotics/pose2@1"),
        vec![
            field("heading", quantity_type()),
            field("position", point2_type()),
        ],
    )
    .expect("reviewed robotics pose schema")
}

pub fn image_region2_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("vision/image-region2@1"),
        vec![field("label", text_type()), field("region", rect2_type())],
    )
    .expect("reviewed image region schema")
}

fn text(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(text_type(), value.as_bytes().to_vec()).expect("bounded text")
}

fn quantity(value: Quantity) -> StructuredInfoValue {
    StructuredInfoValue::leaf(quantity_type(), value.encode().to_vec()).expect("bounded quantity")
}

pub fn point2_value(
    frame: &str,
    x: Quantity,
    y: Quantity,
) -> Result<StructuredInfoValue, GeometryRefusal> {
    require_length(x)?;
    require_length(y)?;
    Ok(StructuredInfoValue::record(
        point2_type(),
        vec![
            value_field("frame", text(frame)),
            value_field("x", quantity(x)),
            value_field("y", quantity(y)),
        ],
    )?)
}

pub fn transform2_value(
    from_frame: &str,
    to_frame: &str,
    offset_x: Quantity,
    offset_y: Quantity,
) -> Result<StructuredInfoValue, GeometryRefusal> {
    require_length(offset_x)?;
    require_length(offset_y)?;
    Ok(StructuredInfoValue::record(
        transform2_type(),
        vec![
            value_field("from_frame", text(from_frame)),
            value_field("offset_x", quantity(offset_x)),
            value_field("offset_y", quantity(offset_y)),
            value_field("to_frame", text(to_frame)),
        ],
    )?)
}

pub fn path2_value(points: Vec<StructuredInfoValue>) -> Result<StructuredInfoValue, GeometryRefusal> {
    let count = u16::try_from(points.len()).map_err(|_| GeometryRefusal::TooManyPoints)?;
    let value_type = path2_type(count)?;
    let points_type = StructuredInfoType::collection(point2_type(), Some(count))?;
    let points = StructuredInfoValue::collection(points_type, points)?;
    Ok(StructuredInfoValue::record(
        value_type,
        vec![value_field("points", points)],
    )?)
}

pub fn apply_transform2(
    point: &StructuredInfoValue,
    transform: &StructuredInfoValue,
) -> Result<StructuredInfoValue, GeometryRefusal> {
    if point.value_type() != &point2_type() || transform.value_type() != &transform2_type() {
        return Err(GeometryRefusal::MalformedInfo);
    }
    let actual = record_text(point, "frame")?;
    let expected = record_text(transform, "from_frame")?;
    if actual != expected {
        return Err(GeometryRefusal::FrameMismatch { expected, actual });
    }
    let x = record_quantity(point, "x")?;
    let y = record_quantity(point, "y")?;
    let offset_x = convert_offset(record_quantity(transform, "offset_x")?, x.unit())?;
    let offset_y = convert_offset(record_quantity(transform, "offset_y")?, y.unit())?;
    let x = Quantity::new(
        x.value()
            .checked_add(offset_x.value())
            .ok_or(GeometryRefusal::Overflow)?,
        x.unit(),
    );
    let y = Quantity::new(
        y.value()
            .checked_add(offset_y.value())
            .ok_or(GeometryRefusal::Overflow)?,
        y.unit(),
    );
    point2_value(&record_text(transform, "to_frame")?, x, y)
}

pub fn apply_transform2_to_path(
    path: &StructuredInfoValue,
    transform: &StructuredInfoValue,
) -> Result<StructuredInfoValue, GeometryRefusal> {
    let points = record_value(path, "points")?;
    let StructuredInfoValueShape::Collection(points) = points.shape() else {
        return Err(GeometryRefusal::MalformedInfo);
    };
    if points.len() > usize::from(MAXIMUM_GEOMETRY_PATH_POINTS) {
        return Err(GeometryRefusal::TooManyPoints);
    }
    path2_value(
        points
            .iter()
            .map(|point| apply_transform2(point, transform))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn require_length(value: Quantity) -> Result<(), GeometryRefusal> {
    (value.dimension() == QuantityDimension::Length)
        .then_some(())
        .ok_or(GeometryRefusal::IncompatibleUnit)
}

fn convert_offset(value: Quantity, unit: QuantityUnit) -> Result<Quantity, GeometryRefusal> {
    require_length(value)?;
    value.convert(unit).map_err(|error| match error {
        QuantityConversionRefusal::IncompatibleDimensions => GeometryRefusal::IncompatibleUnit,
        QuantityConversionRefusal::Inexact => GeometryRefusal::InexactUnitConversion,
        QuantityConversionRefusal::Overflow => GeometryRefusal::Overflow,
    })
}

fn record_value<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, GeometryRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(GeometryRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(GeometryRefusal::MalformedInfo)
}

fn record_text(value: &StructuredInfoValue, name: &str) -> Result<String, GeometryRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = record_value(value, name)?.shape() else {
        return Err(GeometryRefusal::MalformedInfo);
    };
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| GeometryRefusal::MalformedInfo)
}

fn record_quantity(value: &StructuredInfoValue, name: &str) -> Result<Quantity, GeometryRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = record_value(value, name)?.shape() else {
        return Err(GeometryRefusal::MalformedInfo);
    };
    Quantity::decode(bytes).map_err(|_| GeometryRefusal::MalformedInfo)
}
