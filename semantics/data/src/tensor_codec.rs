//! Canonical tensor wire encoding, separate from validation and meaning.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{semantic_digest, BoundedResourceRef, QuantityUnit};

use crate::tensor::*;

impl TensorValue {
    pub fn encode(&self) -> Result<Vec<u8>, TensorRefusal> {
        self.validate()?;
        let mut output = vec![
            TENSOR_ENCODING_VERSION,
            element_tag(self.element),
            self.dimensions.len() as u8,
        ];
        for dimension in &self.dimensions {
            output.extend_from_slice(&dimension.to_le_bytes());
        }
        for axis in &self.axes {
            encode_axis(&mut output, axis)?;
        }
        output.extend_from_slice(&self.content_digest);
        match &self.backing {
            TensorBacking::Inline(payload) => {
                output.push(0);
                output.extend_from_slice(&(payload.len() as u32).to_le_bytes());
                output.extend_from_slice(payload);
            }
            TensorBacking::Resource(reference) => {
                output.push(1);
                let bytes = reference
                    .encode()
                    .map_err(|_| TensorRefusal::InvalidResource)?;
                output.extend_from_slice(&(bytes.len() as u16).to_le_bytes());
                output.extend_from_slice(&bytes);
            }
        }
        Ok(output)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, TensorRefusal> {
        let mut cursor = Cursor::new(encoded);
        if cursor.u8()? != TENSOR_ENCODING_VERSION {
            return Err(TensorRefusal::UnsupportedEncodingVersion);
        }
        let element = decode_element(cursor.u8()?)?;
        let rank = usize::from(cursor.u8()?);
        if rank == 0 || rank > MAXIMUM_TENSOR_RANK {
            return Err(TensorRefusal::RankOutOfBounds);
        }
        let dimensions = (0..rank)
            .map(|_| cursor.u64())
            .collect::<Result<Vec<_>, _>>()?;
        let axes = (0..rank)
            .map(|_| decode_axis(&mut cursor))
            .collect::<Result<Vec<_>, _>>()?;
        let content_digest = cursor.digest()?;
        let backing = match cursor.u8()? {
            0 => TensorBacking::Inline(cursor.bytes_u32()?.to_vec()),
            1 => TensorBacking::Resource(
                BoundedResourceRef::decode(cursor.bytes_u16()?)
                    .map_err(|_| TensorRefusal::InvalidResource)?,
            ),
            _ => return Err(TensorRefusal::MalformedEncoding),
        };
        if !cursor.finished() {
            return Err(TensorRefusal::MalformedEncoding);
        }
        let value = Self {
            element,
            dimensions,
            axes,
            content_digest,
            backing,
        };
        value.validate()?;
        Ok(value)
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], TensorRefusal> {
        self.validate()?;
        let mut identity = vec![
            TENSOR_ENCODING_VERSION,
            element_tag(self.element),
            self.dimensions.len() as u8,
        ];
        for dimension in &self.dimensions {
            identity.extend_from_slice(&dimension.to_le_bytes());
        }
        for axis in &self.axes {
            encode_axis(&mut identity, axis)?;
        }
        identity.extend_from_slice(&self.content_digest);
        Ok(semantic_digest(TENSOR_INFO_ID, &identity))
    }
}

fn element_tag(element: TensorElement) -> u8 {
    match element {
        TensorElement::I8 => 0,
        TensorElement::U8 => 1,
        TensorElement::I16 => 2,
        TensorElement::U16 => 3,
        TensorElement::I32 => 4,
        TensorElement::U32 => 5,
        TensorElement::I64 => 6,
        TensorElement::U64 => 7,
        TensorElement::F32 => 8,
        TensorElement::F64 => 9,
        TensorElement::I24 => 10,
    }
}
fn decode_element(tag: u8) -> Result<TensorElement, TensorRefusal> {
    Ok(match tag {
        0 => TensorElement::I8,
        1 => TensorElement::U8,
        2 => TensorElement::I16,
        3 => TensorElement::U16,
        4 => TensorElement::I32,
        5 => TensorElement::U32,
        6 => TensorElement::I64,
        7 => TensorElement::U64,
        8 => TensorElement::F32,
        9 => TensorElement::F64,
        10 => TensorElement::I24,
        _ => return Err(TensorRefusal::UnsupportedElement),
    })
}

fn encode_axis(output: &mut Vec<u8>, axis: &TensorAxis) -> Result<(), TensorRefusal> {
    match &axis.role {
        TensorAxisRole::Batch => output.push(0),
        TensorAxisRole::Time => output.push(1),
        TensorAxisRole::Feature => output.push(2),
        TensorAxisRole::Sensor => output.push(3),
        TensorAxisRole::SpatialCoordinate => output.push(4),
        TensorAxisRole::Frequency => output.push(5),
        TensorAxisRole::Channel => output.push(6),
        TensorAxisRole::Other(role) => {
            output.push(7);
            push_text(output, role)?;
        }
    }
    push_optional_text(output, axis.identity.as_deref())?;
    match axis.unit {
        None => output.push(0),
        Some(unit) => {
            output.push(1);
            output.push(unit_tag(unit));
        }
    }
    Ok(())
}
fn decode_axis(cursor: &mut Cursor<'_>) -> Result<TensorAxis, TensorRefusal> {
    let role = match cursor.u8()? {
        0 => TensorAxisRole::Batch,
        1 => TensorAxisRole::Time,
        2 => TensorAxisRole::Feature,
        3 => TensorAxisRole::Sensor,
        4 => TensorAxisRole::SpatialCoordinate,
        5 => TensorAxisRole::Frequency,
        6 => TensorAxisRole::Channel,
        7 => TensorAxisRole::Other(cursor.text()?.to_string()),
        _ => return Err(TensorRefusal::UnsupportedAxisRole),
    };
    let identity = match cursor.u8()? {
        0 => None,
        1 => Some(cursor.text()?.to_string()),
        _ => return Err(TensorRefusal::MalformedEncoding),
    };
    let unit = match cursor.u8()? {
        0 => None,
        1 => Some(decode_unit(cursor.u8()?)?),
        _ => return Err(TensorRefusal::MalformedEncoding),
    };
    Ok(TensorAxis {
        role,
        identity,
        unit,
    })
}
fn push_optional_text(output: &mut Vec<u8>, value: Option<&str>) -> Result<(), TensorRefusal> {
    match value {
        None => output.push(0),
        Some(value) => {
            output.push(1);
            push_text(output, value)?;
        }
    }
    Ok(())
}
fn push_text(output: &mut Vec<u8>, value: &str) -> Result<(), TensorRefusal> {
    validate_axis_identity(Some(value))?;
    output.push(value.len() as u8);
    output.extend_from_slice(value.as_bytes());
    Ok(())
}

fn unit_tag(unit: QuantityUnit) -> u8 {
    use QuantityUnit::*;
    match unit {
        Nanosecond => 0,
        Microsecond => 1,
        Millisecond => 2,
        Second => 3,
        Millihertz => 4,
        Hertz => 5,
        Microvolt => 6,
        Millivolt => 7,
        Volt => 8,
        Micrometer => 9,
        Millimeter => 10,
        Centimeter => 11,
        Meter => 12,
        Microdegree => 13,
        Millidegree => 14,
        Degree => 15,
        Millionth => 16,
        Permille => 17,
        Percent => 18,
        One => 19,
        Byte => 20,
        Kibibyte => 21,
        Mebibyte => 22,
    }
}
fn decode_unit(tag: u8) -> Result<QuantityUnit, TensorRefusal> {
    use QuantityUnit::*;
    Ok(match tag {
        0 => Nanosecond,
        1 => Microsecond,
        2 => Millisecond,
        3 => Second,
        4 => Millihertz,
        5 => Hertz,
        6 => Microvolt,
        7 => Millivolt,
        8 => Volt,
        9 => Micrometer,
        10 => Millimeter,
        11 => Centimeter,
        12 => Meter,
        13 => Microdegree,
        14 => Millidegree,
        15 => Degree,
        16 => Millionth,
        17 => Permille,
        18 => Percent,
        19 => One,
        20 => Byte,
        21 => Kibibyte,
        22 => Mebibyte,
        _ => return Err(TensorRefusal::UnsupportedUnit),
    })
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    fn take(&mut self, count: usize) -> Result<&'a [u8], TensorRefusal> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(TensorRefusal::MalformedEncoding)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(TensorRefusal::MalformedEncoding)?;
        self.offset = end;
        Ok(value)
    }
    fn u8(&mut self) -> Result<u8, TensorRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, TensorRefusal> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, TensorRefusal> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, TensorRefusal> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn digest(&mut self) -> Result<[u8; 32], TensorRefusal> {
        self.take(32)?
            .try_into()
            .map_err(|_| TensorRefusal::MalformedEncoding)
    }
    fn text(&mut self) -> Result<&'a str, TensorRefusal> {
        let len = usize::from(self.u8()?);
        core::str::from_utf8(self.take(len)?).map_err(|_| TensorRefusal::MalformedEncoding)
    }
    fn bytes_u16(&mut self) -> Result<&'a [u8], TensorRefusal> {
        let len = usize::from(self.u16()?);
        self.take(len)
    }
    fn bytes_u32(&mut self) -> Result<&'a [u8], TensorRefusal> {
        let len = usize::try_from(self.u32()?).map_err(|_| TensorRefusal::MalformedEncoding)?;
        self.take(len)
    }
    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
