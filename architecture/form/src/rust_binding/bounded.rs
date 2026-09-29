/// Allocation-free storage for a semantically bounded sequence.
///
/// `Option<T>` is representation machinery only; it is never semantic truth
/// and never appears in canonical Conduit encoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSequence<T, const MAXIMUM: usize> {
    values: [Option<T>; MAXIMUM],
    length: usize,
}

impl<T, const MAXIMUM: usize> BoundedSequence<T, MAXIMUM> {
    pub fn new() -> Self {
        Self {
            values: core::array::from_fn(|_| None),
            length: 0,
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), T> {
        let Some(slot) = self.values.get_mut(self.length) else {
            return Err(value);
        };
        *slot = Some(value);
        self.length += 1;
        Ok(())
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn capacity(&self) -> usize {
        MAXIMUM
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &T> {
        self.values[..self.length]
            .iter()
            .map(|value| value.as_ref().expect("active bounded-sequence prefix"))
    }
}

impl<T, const MAXIMUM: usize> Default for BoundedSequence<T, MAXIMUM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const MAXIMUM: usize> IntoIterator for BoundedSequence<T, MAXIMUM> {
    type Item = T;
    type IntoIter = core::iter::Flatten<core::array::IntoIter<Option<T>, MAXIMUM>>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter().flatten()
    }
}

/// Allocation-free finite bytes used when a checked Type permits at most `N`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedBytes<const MAXIMUM: usize> {
    bytes: [u8; MAXIMUM],
    length: usize,
}

impl<const MAXIMUM: usize> BoundedBytes<MAXIMUM> {
    pub fn new(value: &[u8]) -> Option<Self> {
        if value.len() > MAXIMUM {
            return None;
        }
        let mut bytes = [0; MAXIMUM];
        bytes[..value.len()].copy_from_slice(value);
        Some(Self {
            bytes,
            length: value.len(),
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..self.length]
    }

    pub const fn capacity(&self) -> usize {
        MAXIMUM
    }
}

/// Allocation-free finite UTF-8 whose semantic bound is expressed in bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedText<const MAXIMUM: usize>(BoundedBytes<MAXIMUM>);

impl<const MAXIMUM: usize> BoundedText<MAXIMUM> {
    pub fn new(value: &str) -> Option<Self> {
        BoundedBytes::new(value.as_bytes()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(self.0.as_slice()).expect("constructed from valid UTF-8")
    }

    pub const fn capacity(&self) -> usize {
        MAXIMUM
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{vec, vec::Vec};

    #[test]
    fn sequence_and_text_are_fixed_capacity_and_fail_closed() {
        let mut values = BoundedSequence::<u8, 2>::new();
        values.push(3).unwrap();
        values.push(5).unwrap();
        assert_eq!(values.push(8), Err(8));
        assert_eq!(values.iter().copied().collect::<Vec<_>>(), vec![3, 5]);

        let text = BoundedText::<4>::new("é").unwrap();
        assert_eq!(text.as_str(), "é");
        assert!(BoundedText::<1>::new("é").is_none());
    }
}
