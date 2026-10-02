/// Storage for a semantically bounded sequence.
///
/// The exact payload is heap-backed so a generous authored maximum does not
/// inflate every generated value's stack frame. `MAXIMUM` remains the semantic
/// capacity and is enforced on every construction path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedSequence<T, const MAXIMUM: usize> {
    values: alloc::vec::Vec<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundedSequenceCapacityRefusal {
    MaximumExceeded,
}

impl<T, const MAXIMUM: usize> BoundedSequence<T, MAXIMUM> {
    pub fn new() -> Self {
        Self {
            values: alloc::vec::Vec::new(),
        }
    }

    pub fn push(&mut self, value: T) -> Result<(), T> {
        if self.values.len() == MAXIMUM {
            return Err(value);
        }
        self.values.push(value);
        Ok(())
    }

    pub fn try_from_iter(
        values: impl IntoIterator<Item = T>,
    ) -> Result<Self, BoundedSequenceCapacityRefusal> {
        let mut bounded = Self::new();
        for value in values {
            bounded
                .push(value)
                .map_err(|_| BoundedSequenceCapacityRefusal::MaximumExceeded)?;
        }
        Ok(bounded)
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub const fn capacity(&self) -> usize {
        MAXIMUM
    }

    pub fn iter(&self) -> core::slice::Iter<'_, T> {
        self.values.iter()
    }

    pub fn last(&self) -> Option<&T> {
        self.values.last()
    }

    pub fn contains(&self, value: &T) -> bool
    where
        T: PartialEq,
    {
        self.values.contains(value)
    }

    pub fn binary_search(&self, value: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        let mut left = 0;
        let mut right = self.values.len();
        while left < right {
            let middle = left + (right - left) / 2;
            match self[middle].cmp(value) {
                core::cmp::Ordering::Less => left = middle + 1,
                core::cmp::Ordering::Greater => right = middle,
                core::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        Err(left)
    }

    pub fn binary_search_by_key<B: Ord>(
        &self,
        key: &B,
        mut projection: impl FnMut(&T) -> B,
    ) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.values.len();
        while left < right {
            let middle = left + (right - left) / 2;
            match projection(&self[middle]).cmp(key) {
                core::cmp::Ordering::Less => left = middle + 1,
                core::cmp::Ordering::Greater => right = middle,
                core::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        Err(left)
    }
}

impl<T, const MAXIMUM: usize> Default for BoundedSequence<T, MAXIMUM> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const MAXIMUM: usize> IntoIterator for BoundedSequence<T, MAXIMUM> {
    type Item = T;
    type IntoIter = alloc::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}

impl<'a, T, const MAXIMUM: usize> IntoIterator for &'a BoundedSequence<T, MAXIMUM> {
    type Item = &'a T;
    type IntoIter = core::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.iter()
    }
}

impl<T, const MAXIMUM: usize> core::ops::Index<usize> for BoundedSequence<T, MAXIMUM> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl<T: serde::Serialize, const MAXIMUM: usize> serde::Serialize for BoundedSequence<T, MAXIMUM> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.iter())
    }
}

impl<'de, T: serde::Deserialize<'de>, const MAXIMUM: usize> serde::Deserialize<'de>
    for BoundedSequence<T, MAXIMUM>
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<T, const MAXIMUM: usize>(core::marker::PhantomData<T>);

        impl<'de, T: serde::Deserialize<'de>, const MAXIMUM: usize> serde::de::Visitor<'de>
            for Visitor<T, MAXIMUM>
        {
            type Value = BoundedSequence<T, MAXIMUM>;

            fn expecting(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(formatter, "at most {MAXIMUM} values")
            }

            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut result = BoundedSequence::new();
                while let Some(value) = sequence.next_element()? {
                    result
                        .push(value)
                        .map_err(|_| serde::de::Error::invalid_length(MAXIMUM + 1, &self))?;
                }
                Ok(result)
            }
        }

        deserializer.deserialize_seq(Visitor::<T, MAXIMUM>(core::marker::PhantomData))
    }
}

use alloc::boxed::Box;

/// Finite bytes used when a checked Type permits at most `N`.
/// The exact payload is boxed once at construction so large portable bounds do
/// not turn every generated value into an enormous stack frame or allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedBytes<const MAXIMUM: usize> {
    bytes: Box<[u8]>,
}

impl<const MAXIMUM: usize> BoundedBytes<MAXIMUM> {
    pub fn new(value: &[u8]) -> Option<Self> {
        if value.len() > MAXIMUM {
            return None;
        }
        Some(Self {
            bytes: value.into(),
        })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.bytes
    }

    pub const fn capacity(&self) -> usize {
        MAXIMUM
    }
}

impl<const MAXIMUM: usize> serde::Serialize for BoundedBytes<MAXIMUM> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bytes(self.as_slice())
    }
}

impl<'de, const MAXIMUM: usize> serde::Deserialize<'de> for BoundedBytes<MAXIMUM> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <alloc::vec::Vec<u8> as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(&value).ok_or_else(|| serde::de::Error::custom("byte value exceeds its bound"))
    }
}

/// Finite UTF-8 whose semantic bound is expressed in bytes.
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

impl<const MAXIMUM: usize> serde::Serialize for BoundedText<MAXIMUM> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de, const MAXIMUM: usize> serde::Deserialize<'de> for BoundedText<MAXIMUM> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <alloc::string::String as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(&value).ok_or_else(|| serde::de::Error::custom("text value exceeds its bound"))
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
        assert_eq!(
            BoundedSequence::<u8, 2>::try_from_iter([3, 5])
                .unwrap()
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![3, 5]
        );
        assert_eq!(
            BoundedSequence::<u8, 2>::try_from_iter([3, 5, 8]),
            Err(BoundedSequenceCapacityRefusal::MaximumExceeded)
        );

        let text = BoundedText::<4>::new("é").unwrap();
        assert_eq!(text.as_str(), "é");
        assert!(BoundedText::<1>::new("é").is_none());
    }
}
