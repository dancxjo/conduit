//! Borrowed canonical addressing without copying a separately retained prefix.
#[derive(Clone, Copy)]
pub(super) enum CanonicalInput<'a> {
    Whole(&'a [u8]),
    Separated { schema: &'a [u8], node: &'a [u8] },
}
impl<'a> CanonicalInput<'a> {
    pub(super) fn len(self) -> Option<usize> {
        match self {
            Self::Whole(bytes) => Some(bytes.len()),
            Self::Separated { schema, node } => schema.len().checked_add(node.len()),
        }
    }
    pub(super) fn whole(self) -> Option<&'a [u8]> {
        match self {
            Self::Whole(bytes) => Some(bytes),
            _ => None,
        }
    }
    pub(super) fn get(self, range: core::ops::Range<usize>) -> Option<&'a [u8]> {
        if range.start > range.end {
            return None;
        }
        match self {
            Self::Whole(bytes) => bytes.get(range),
            Self::Separated { schema, node } => {
                if range.end <= schema.len() {
                    schema.get(range)
                } else {
                    node.get(
                        range.start.checked_sub(schema.len())?
                            ..range.end.checked_sub(schema.len())?,
                    )
                }
            }
        }
    }
}
