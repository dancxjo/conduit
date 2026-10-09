//! Canonical semantic payload codecs used by model-work envelopes.
use super::*;

pub(crate) mod native {
    use super::*;
    pub fn serialize<T: NativeRustBinding + Clone, S: serde::Serializer>(
        value: &T,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .clone()
            .encode()
            .map_err(|_| serde::ser::Error::custom("invalid semantic payload"))?
            .serialize(serializer)
    }
    pub fn deserialize<'de, T: NativeRustBinding, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<T, D::Error> {
        T::decode(&Vec::<u8>::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid semantic payload"))
    }
}
pub(crate) mod native_vec {
    use super::*;
    pub fn serialize<T: NativeRustBinding + Clone, S: serde::Serializer>(
        values: &[T],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        values
            .iter()
            .cloned()
            .map(NativeRustBinding::encode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| serde::ser::Error::custom("invalid semantic payload"))?
            .serialize(serializer)
    }
    pub fn deserialize<'de, T: NativeRustBinding, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<T>, D::Error> {
        Vec::<Vec<u8>>::deserialize(deserializer)?
            .iter()
            .map(|bytes| {
                T::decode(bytes).map_err(|_| serde::de::Error::custom("invalid semantic payload"))
            })
            .collect()
    }
}
pub(super) mod tensors {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        values: &[TensorValue],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        tensor_bounds(values).map_err(|_| serde::ser::Error::custom("tensor bounds"))?;
        values
            .iter()
            .map(TensorValue::encode)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| serde::ser::Error::custom("invalid tensor"))?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<TensorValue>, D::Error> {
        let bytes = Vec::<Vec<u8>>::deserialize(deserializer)?;
        if bytes.is_empty() || bytes.len() > MODEL_WORK_MAXIMUM_TENSORS {
            return Err(serde::de::Error::custom("tensor bounds"));
        }
        bytes
            .iter()
            .map(|bytes| {
                TensorValue::decode(bytes).map_err(|_| serde::de::Error::custom("invalid tensor"))
            })
            .collect()
    }
}
pub(crate) mod resource {
    use super::*;
    pub fn serialize<S: serde::Serializer>(
        value: &BoundedResourceRef,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value
            .encode()
            .map_err(|_| serde::ser::Error::custom("invalid resource"))?
            .serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<BoundedResourceRef, D::Error> {
        BoundedResourceRef::decode(&Vec::<u8>::deserialize(deserializer)?)
            .map_err(|_| serde::de::Error::custom("invalid resource"))
    }
}
