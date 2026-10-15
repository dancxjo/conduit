//! Bounded preparation-time ONNX signature inspection. No graph execution or I/O.
//!
//! The first profile excludes attributes, sparse tensors, local functions and
//! training graphs. This prevents uninspected nested external tensor references.

use crate::onnx_wire::{Fields, Value};
use alloc::{string::String, vec::Vec};
use conduit_data::TensorElement;
use sha2::{Digest, Sha256};

pub const MAXIMUM_ONNX_BYTES: usize = 16 * 1024 * 1024;
const MAXIMUM_FIELDS: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxMetadataRefusal {
    InvalidVersion,
    ByteBound,
    Malformed,
    Unsupported,
    MissingField,
    DuplicateField,
    PortBound,
    InvalidName,
    InvalidDimension,
    ExternalData,
    DigestMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnnxDimension {
    Fixed(u64),
    Symbol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxPort {
    pub name: String,
    pub element: TensorElement,
    pub dimensions: Vec<OnnxDimension>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OnnxMetadata {
    pub sha256: [u8; 32],
    pub ir_version: u64,
    pub opsets: Vec<(String, u64)>,
    pub graph_name: String,
    pub inputs: Vec<OnnxPort>,
    pub outputs: Vec<OnnxPort>,
}

type Refusal = OnnxMetadataRefusal;

/// Inspect only caller-supplied bytes; the digest is the raw file SHA-256.
/// Successful inspection is not operator validation or execution admission.
pub fn inspect_onnx(bytes: &[u8], expected_sha256: [u8; 32]) -> Result<OnnxMetadata, Refusal> {
    if bytes.is_empty() || bytes.len() > MAXIMUM_ONNX_BYTES {
        return Err(Refusal::ByteBound);
    }
    let sha256: [u8; 32] = Sha256::digest(bytes).into();
    if sha256 != expected_sha256 {
        return Err(Refusal::DigestMismatch);
    }
    let mut ir = None;
    let mut graph = None;
    let mut opsets = Vec::new();
    visit(bytes, |field, value| {
        match field {
            1 => set(&mut ir, version_number(value)?)?,
            7 => set(&mut graph, raw(value)?)?,
            8 => {
                if opsets.len() == 8 {
                    return Err(Refusal::Unsupported);
                }
                let mut domain = None;
                let mut version = None;
                visit(raw(value)?, |f, v| {
                    match f {
                        1 => set(&mut domain, text(v, true)?)?,
                        2 => set(&mut version, version_number(v)?)?,
                        _ => {}
                    }
                    Ok(())
                })?;
                let domain = domain.unwrap_or_default();
                if opsets.iter().any(|(d, _)| d == &domain) {
                    return Err(Refusal::DuplicateField);
                }
                opsets.push((domain, version.ok_or(Refusal::MissingField)?));
            }
            20 | 25 | 26 => return Err(Refusal::Unsupported),
            _ => {}
        }
        Ok(())
    })?;
    if opsets.is_empty() {
        return Err(Refusal::MissingField);
    }
    let ir_version = ir.ok_or(Refusal::MissingField)?;
    let mut result = OnnxMetadata {
        sha256,
        ir_version,
        opsets,
        graph_name: String::new(),
        inputs: Vec::new(),
        outputs: Vec::new(),
    };
    let mut graph_name = None;
    visit(graph.ok_or(Refusal::MissingField)?, |field, value| {
        match field {
            1 => visit(raw(value)?, |f, _| {
                if matches!(f, 5 | 10) {
                    Err(Refusal::Unsupported)
                } else {
                    Ok(())
                }
            })?,
            2 => set(&mut graph_name, text(value, false)?)?,
            5 => visit(raw(value)?, |f, v| {
                if f == 13 || (f == 14 && integer(v)? != 0) {
                    Err(Refusal::ExternalData)
                } else {
                    Ok(())
                }
            })?,
            11 | 12 => {
                if result.inputs.len() + result.outputs.len() == 32 {
                    return Err(Refusal::PortBound);
                }
                let port = port(raw(value)?)?;
                let ports = if field == 11 {
                    &mut result.inputs
                } else {
                    &mut result.outputs
                };
                if ports.iter().any(|p| p.name == port.name) {
                    return Err(Refusal::DuplicateField);
                }
                ports.push(port);
            }
            15 => return Err(Refusal::Unsupported),
            _ => {}
        }
        Ok(())
    })?;
    result.graph_name = graph_name.ok_or(Refusal::MissingField)?;
    if result.inputs.is_empty() || result.outputs.is_empty() {
        return Err(Refusal::MissingField);
    }
    Ok(result)
}

fn port(bytes: &[u8]) -> Result<OnnxPort, Refusal> {
    let mut name = None;
    let mut tensor = None;
    visit(bytes, |f, v| {
        match f {
            1 => set(&mut name, text(v, false)?)?,
            2 => visit(raw(v)?, |f, v| {
                if f == 1 {
                    set(&mut tensor, raw(v)?)
                } else if matches!(f, 4 | 5 | 7 | 8 | 9) {
                    Err(Refusal::Unsupported)
                } else {
                    Ok(())
                }
            })?,
            _ => {}
        }
        Ok(())
    })?;
    let mut element = None;
    let mut shape = None;
    visit(tensor.ok_or(Refusal::MissingField)?, |f, v| {
        match f {
            1 => set(&mut element, dtype(integer(v)?)?)?,
            2 => set(&mut shape, raw(v)?)?,
            _ => {}
        }
        Ok(())
    })?;
    let mut dimensions = Vec::new();
    visit(shape.ok_or(Refusal::MissingField)?, |f, v| {
        if f == 1 {
            if dimensions.len() == 8 {
                return Err(Refusal::InvalidDimension);
            }
            let mut dimension = None;
            visit(raw(v)?, |f, v| {
                match f {
                    1 => {
                        let n = positive(v)?;
                        if n > i64::MAX as u64 {
                            return Err(Refusal::InvalidDimension);
                        }
                        set(&mut dimension, OnnxDimension::Fixed(n))?;
                    }
                    2 => set(&mut dimension, OnnxDimension::Symbol(text(v, false)?))?,
                    _ => {}
                }
                Ok(())
            })?;
            dimensions.push(dimension.ok_or(Refusal::InvalidDimension)?);
        }
        Ok(())
    })?;
    // Existing Data TensorValue requires rank >= 1; never silently reshape scalars.
    if dimensions.is_empty() {
        return Err(Refusal::Unsupported);
    }
    Ok(OnnxPort {
        name: name.ok_or(Refusal::MissingField)?,
        element: element.ok_or(Refusal::MissingField)?,
        dimensions,
    })
}

fn dtype(value: u64) -> Result<TensorElement, Refusal> {
    Ok(match value {
        1 => TensorElement::F32,
        2 => TensorElement::U8,
        3 => TensorElement::I8,
        4 => TensorElement::U16,
        5 => TensorElement::I16,
        6 => TensorElement::I32,
        7 => TensorElement::I64,
        11 => TensorElement::F64,
        12 => TensorElement::U32,
        13 => TensorElement::U64,
        _ => return Err(Refusal::Unsupported),
    })
}

fn visit<'a>(
    bytes: &'a [u8],
    mut f: impl FnMut(u32, Value<'a>) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let mut fields = Fields::new(bytes, MAXIMUM_FIELDS);
    while let Some((field, value)) = fields.next().map_err(|_| Refusal::Malformed)? {
        f(field, value)?;
    }
    Ok(())
}
fn raw(value: Value<'_>) -> Result<&[u8], Refusal> {
    if let Value::Bytes(bytes) = value {
        Ok(bytes)
    } else {
        Err(Refusal::Malformed)
    }
}
fn integer(value: Value<'_>) -> Result<u64, Refusal> {
    if let Value::Integer(n) = value {
        Ok(n)
    } else {
        Err(Refusal::Malformed)
    }
}
fn positive(value: Value<'_>) -> Result<u64, Refusal> {
    let n = integer(value)?;
    if n == 0 {
        Err(Refusal::InvalidDimension)
    } else {
        Ok(n)
    }
}
fn version_number(value: Value<'_>) -> Result<u64, Refusal> {
    let n = integer(value)?;
    if n == 0 || n > i64::MAX as u64 {
        Err(Refusal::InvalidVersion)
    } else {
        Ok(n)
    }
}
fn text(value: Value<'_>, empty: bool) -> Result<String, Refusal> {
    let bytes = raw(value)?;
    if bytes.len() > 128 || (!empty && bytes.is_empty()) || bytes.contains(&0) {
        return Err(Refusal::InvalidName);
    }
    core::str::from_utf8(bytes)
        .map(String::from)
        .map_err(|_| Refusal::InvalidName)
}
fn set<T>(slot: &mut Option<T>, value: T) -> Result<(), Refusal> {
    if slot.is_some() {
        Err(Refusal::DuplicateField)
    } else {
        *slot = Some(value);
        Ok(())
    }
}
