//! Allocation-free traversal of exact canonical Types retained by Native family
//! descriptors. This handles representation framing, never parser policy/laws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SchemaRefusal;
#[derive(Clone, Copy)]
pub(crate) struct Fields<'a> {
    remaining: &'a [u8],
    count: usize,
}
pub(crate) enum Shape<'a> {
    Leaf(&'a str),
    Collection { element: &'a [u8], length: u16 },
    Sequence,
    Nominal { representation: &'a [u8] },
    Record(Fields<'a>),
    Variant(Fields<'a>),
}
fn take<'a>(input: &mut &'a [u8], count: usize) -> Result<&'a [u8], SchemaRefusal> {
    let (part, rest) = input.split_at_checked(count).ok_or(SchemaRefusal)?;
    *input = rest;
    Ok(part)
}
fn u16(input: &mut &[u8]) -> Result<u16, SchemaRefusal> {
    Ok(u16::from_le_bytes(
        take(input, 2)?.try_into().map_err(|_| SchemaRefusal)?,
    ))
}
fn count(input: &mut &[u8]) -> Result<usize, SchemaRefusal> {
    usize::try_from(u32::from_le_bytes(
        take(input, 4)?.try_into().map_err(|_| SchemaRefusal)?,
    ))
    .map_err(|_| SchemaRefusal)
}
fn text<'a>(input: &mut &'a [u8]) -> Result<&'a str, SchemaRefusal> {
    let length = count(input)?;
    core::str::from_utf8(take(input, length)?).map_err(|_| SchemaRefusal)
}
fn skip(input: &mut &[u8], depth: usize, nodes: &mut usize) -> Result<(), SchemaRefusal> {
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH || *nodes == 0 {
        return Err(SchemaRefusal);
    }
    *nodes -= 1;
    match take(input, 1)?[0] {
        0 => {
            text(input)?;
        }
        1 => {
            u16(input)?;
            skip(input, depth + 1, nodes)?;
        }
        2 | 3 => {
            text(input)?;
            let length = count(input)?;
            if length > *nodes {
                return Err(SchemaRefusal);
            }
            for _ in 0..length {
                text(input)?;
                skip(input, depth + 1, nodes)?;
            }
        }
        5 => {
            text(input)?;
            skip(input, depth + 1, nodes)?;
        }
        4 => {
            u16(input)?;
            u16(input)?;
            skip(input, depth + 1, nodes)?;
        }
        _ => return Err(SchemaRefusal),
    }
    Ok(())
}
fn child<'a>(input: &mut &'a [u8]) -> Result<&'a [u8], SchemaRefusal> {
    let original = *input;
    let mut nodes = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
    skip(input, 1, &mut nodes)?;
    Ok(&original[..original.len() - input.len()])
}
impl<'a> Iterator for Fields<'a> {
    type Item = Result<(&'a str, &'a [u8]), SchemaRefusal>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1;
        Some((|| {
            Ok((text(&mut self.remaining)?, child(&mut self.remaining)?))
        })())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.count, Some(self.count))
    }
}
impl ExactSizeIterator for Fields<'_> {}
pub(crate) fn shape(mut encoded: &[u8]) -> Result<Shape<'_>, SchemaRefusal> {
    let original = encoded;
    let complete = child(&mut encoded)?;
    if !encoded.is_empty() || complete.len() != original.len() {
        return Err(SchemaRefusal);
    }
    encoded = original;
    Ok(match take(&mut encoded, 1)?[0] {
        0 => Shape::Leaf(text(&mut encoded)?),
        1 => {
            let length = u16(&mut encoded)?;
            Shape::Collection {
                length,
                element: child(&mut encoded)?,
            }
        }
        2 | 3 => {
            let tag = original[0];
            text(&mut encoded)?;
            let count = count(&mut encoded)?;
            let fields = Fields {
                remaining: encoded,
                count,
            };
            if tag == 2 {
                Shape::Record(fields)
            } else {
                Shape::Variant(fields)
            }
        }
        4 => Shape::Sequence,
        5 => {
            text(&mut encoded)?;
            Shape::Nominal {
                representation: child(&mut encoded)?,
            }
        }
        _ => return Err(SchemaRefusal),
    })
}
pub(crate) fn select_field<'a>(
    mut encoded: &'a [u8],
    path: &[&str],
) -> Result<&'a [u8], SchemaRefusal> {
    for name in path {
        let Shape::Record(fields) = shape(encoded)? else {
            return Err(SchemaRefusal);
        };
        let mut selected = None;
        for field in fields {
            let (field_name, ty) = field?;
            if field_name == *name {
                selected = Some(ty);
            }
        }
        encoded = selected.ok_or(SchemaRefusal)?;
    }
    Ok(encoded)
}
