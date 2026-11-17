//! Experimental structural scan only: no authority or allocation grant.
use super::{ImageBounds, ImageLayout};
use serde::de::{DeserializeSeed, Error, SeqAccess, Visitor};
use std::fmt;
pub(super) fn inspect(input: &str, bounds: &ImageBounds) -> Result<ImageLayout, serde_json::Error> {
    let mut layout = ImageLayout::default();
    let mut decoder = serde_json::Deserializer::from_str(input);
    Images(&mut layout, bounds).deserialize(&mut decoder)?;
    decoder.end()?;
    Ok(layout)
}
struct Images<'a>(&'a mut ImageLayout, &'a ImageBounds);
impl<'de> DeserializeSeed<'de> for Images<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        decoder.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Images<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of image byte arrays")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<(), A::Error> {
        while array
            .next_element_seed(Image(&mut *self.0, self.1))?
            .is_some()
        {}
        Ok(())
    }
}
struct Image<'a>(&'a mut ImageLayout, &'a ImageBounds);
impl<'de> DeserializeSeed<'de> for Image<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<(), D::Error> {
        if self.0.images == self.1.maximum_images {
            return Err(D::Error::custom("image count exceeds profile"));
        }
        decoder.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Image<'_> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("an array of integer bytes")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<(), A::Error> {
        let mut bytes = 0;
        while array.next_element::<u8>()?.is_some() {
            if self.0.compressed_bytes == self.1.maximum_compressed_bytes {
                return Err(A::Error::custom("compressed image bytes exceed profile"));
            }
            self.0.compressed_bytes += 1;
            bytes += 1;
        }
        self.0.images += 1;
        self.0.largest_image = self.0.largest_image.max(bytes);
        Ok(())
    }
}

pub(super) fn inspect_token(input: &str) -> Result<usize, serde_json::Error> {
    struct Bytes;
    impl<'de> Visitor<'de> for Bytes {
        type Value = usize;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an array of integer bytes")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<usize, A::Error> {
            let mut count = 0usize;
            while array.next_element::<u8>()?.is_some() {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| A::Error::custom("token extent overflow"))?;
            }
            Ok(count)
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(input);
    let count = serde::Deserializer::deserialize_seq(&mut decoder, Bytes)?;
    decoder.end()?;
    Ok(count)
}
