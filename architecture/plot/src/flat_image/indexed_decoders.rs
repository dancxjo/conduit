//! Trusted index construction and explicit JSON or linked Make decode drivers.
//! Selecting a linked driver never installs or references rich JSON decoding.
use super::*;
use serde::{
    de::{Error, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

impl<'a> IndexedSourceImage<'a> {
    /// Validate the index and use the ordinary JSON-backed Gear profile.
    /// # Safety
    /// Authenticate the complete immutable bytes and checked Make origin first.
    /// Bounds/index validation is not Source checking or received-input trust.
    pub unsafe fn from_make_image(bytes: &'a [u8]) -> Result<Self, E> {
        Self::validated(bytes, json_gear, json_configuration, None, None)
    }
    /// Validate the same index, using exact constructors from its trusted Make.
    /// This path does not retain a fallback rich JSON Gear decoder.
    /// # Safety
    /// Authenticate the complete immutable image independently. Both callbacks
    /// must reconstruct exactly every corresponding Gear and its configuration
    /// from this same checked Make, including all fields and ordering. Received
    /// callbacks, claimed identities, or matching index bounds cannot prove it.
    pub unsafe fn from_make_image_with_decoders(
        bytes: &'a [u8],
        gear: fn(usize) -> Result<CheckedGear, E>,
        configuration: fn(usize) -> Result<IndexedGearConfiguration, E>,
    ) -> Result<Self, E> {
        Self::validated(
            bytes,
            linked_gear,
            linked_configuration,
            Some(gear),
            Some(configuration),
        )
    }
    fn validated(
        bytes: &'a [u8],
        gear_driver: fn(&IndexedSourceImage<'_>, usize) -> Result<CheckedGear, E>,
        configuration_driver: fn(
            &IndexedSourceImage<'_>,
            usize,
        ) -> Result<IndexedGearConfiguration, E>,
        gear_decoder: Option<fn(usize) -> Result<CheckedGear, E>>,
        configuration_decoder: Option<fn(usize) -> Result<IndexedGearConfiguration, E>>,
    ) -> Result<Self, E> {
        if bytes.len() > MAXIMUM_BYTES {
            return Err(E::Oversized);
        }
        if bytes.get(..8) != Some(MAGIC) || bytes.get(10..12) != Some(&[0, 0]) {
            return Err(E::Representation);
        }
        let count = usize::from(u16::from_le_bytes(
            bytes
                .get(8..10)
                .ok_or(E::Representation)?
                .try_into()
                .unwrap(),
        ));
        if count == 0 || count > MAXIMUM_GEARS {
            return Err(E::UnsupportedGraph);
        }
        let metadata_start = PREFIX + (count + 1) * 4;
        let length = word(bytes, 12)?;
        if length > MAXIMUM_METADATA_BYTES {
            return Err(E::Oversized);
        }
        let payload_start = metadata_start
            .checked_add(length)
            .filter(|n| *n <= bytes.len())
            .ok_or(E::Representation)?;
        let mut previous = word(bytes, PREFIX)?;
        if previous != 0 {
            return Err(E::Representation);
        }
        for i in 1..=count {
            let end = word(bytes, PREFIX + i * 4)?;
            if end <= previous
                || end - previous > MAXIMUM_GEAR_BYTES
                || end > bytes.len() - payload_start
            {
                return Err(E::Representation);
            }
            previous = end;
        }
        if previous != bytes.len() - payload_start {
            return Err(E::Representation);
        }
        Ok(Self {
            bytes,
            planning_facts: None,
            gear_driver,
            configuration_driver,
            gear_decoder,
            configuration_decoder,
            count,
            metadata_start,
            payload_start,
        })
    }

    /// Decode only exact configuration; no unrelated port/semantic contracts.
    pub fn gear_configuration(&self, index: usize) -> Result<IndexedGearConfiguration, E> {
        if index >= self.count {
            return Err(E::Representation);
        }
        (self.configuration_driver)(self, index)
    }
    pub fn gear(&self, index: usize) -> Result<CheckedGear, E> {
        if index >= self.count {
            return Err(E::Representation);
        }
        (self.gear_driver)(self, index)
    }
}
fn payload<'a>(image: &IndexedSourceImage<'a>, index: usize) -> Result<&'a [u8], E> {
    let start = word(image.bytes, PREFIX + index * 4)?;
    let end = word(image.bytes, PREFIX + (index + 1) * 4)?;
    Ok(&image.bytes[image.payload_start + start..image.payload_start + end])
}
fn json_gear(image: &IndexedSourceImage<'_>, index: usize) -> Result<CheckedGear, E> {
    serde_json::from_slice(payload(image, index)?).map_err(|_| E::Representation)
}
fn json_configuration(
    image: &IndexedSourceImage<'_>,
    index: usize,
) -> Result<IndexedGearConfiguration, E> {
    let mut value: IndexedGearConfiguration =
        serde_json::from_slice(payload(image, index)?).map_err(|_| E::Representation)?;
    value.configuration.shrink_to_fit();
    Ok(value)
}
pub(super) fn linked_gear(image: &IndexedSourceImage<'_>, index: usize) -> Result<CheckedGear, E> {
    image.gear_decoder.ok_or(E::Representation)?(index)
}
pub(super) fn linked_configuration(
    image: &IndexedSourceImage<'_>,
    index: usize,
) -> Result<IndexedGearConfiguration, E> {
    image.configuration_decoder.ok_or(E::Representation)?(index)
}
/// Indexed metadata has no Gears: all complete Gears live in indexed payloads.
/// Refuse the first element before materializing even one nested contract.
pub(super) struct EmptyGears;
impl EmptyGears {
    pub(super) fn is_empty(&self) -> bool {
        true
    }
}
struct RejectGear;
impl<'de> Deserialize<'de> for RejectGear {
    fn deserialize<D: Deserializer<'de>>(_: D) -> Result<Self, D::Error> {
        Err(D::Error::custom("indexed metadata Gears must be empty"))
    }
}
impl<'de> Deserialize<'de> for EmptyGears {
    fn deserialize<D: Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Empty;
        impl<'de> Visitor<'de> for Empty {
            type Value = EmptyGears;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("an empty indexed metadata Gear array")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<EmptyGears, A::Error> {
                if sequence.next_element::<RejectGear>()?.is_some() {
                    return Err(A::Error::custom("indexed metadata Gears must be empty"));
                }
                Ok(EmptyGears)
            }
        }
        decoder.deserialize_seq(Empty)
    }
}
