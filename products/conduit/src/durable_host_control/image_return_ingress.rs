//! Borrowed image-return preparation, fenced from installed decoding until its
//! original-counter ingress workspace exists. Raw spans confer no authority.
//! Rejections never include peer-controlled strings in diagnostic formatting.
use serde::{
    de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor},
    Deserialize,
};
use serde_json::value::RawValue;
use std::fmt;

struct Tag(&'static [&'static str]);
impl<'de> DeserializeSeed<'de> for Tag {
    type Value = usize;
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<usize, D::Error> {
        decoder.deserialize_str(self)
    }
}
impl<'de> Visitor<'de> for Tag {
    type Value = usize;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a known field or tag")
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<usize, E> {
        self.0
            .iter()
            .position(|expected| *expected == value)
            .ok_or_else(|| E::custom("unknown image-return field or tag"))
    }
}

fn require_shape<E: Error>(raw: &RawValue, allowed: &[u8]) -> Result<(), E> {
    let first = raw
        .get()
        .bytes()
        .find(|byte| !matches!(byte, b' ' | b'\n' | b'\r' | b'\t'));
    if first.is_some_and(|byte| allowed.contains(&byte)) {
        Ok(())
    } else {
        Err(E::custom("unexpected image-return JSON shape"))
    }
}
fn validated_span<'a>(input: &'a str, allowed: &[u8]) -> Result<&'a RawValue, serde_json::Error> {
    let raw = serde_json::from_str::<&RawValue>(input)?;
    require_shape(raw, allowed)?;
    Ok(raw)
}
// Map keys are already JSON strings. Value tags need a raw shape check before
// deserialize_str: serde's wrong-type numeric path can otherwise invoke lexical.
struct ValueTag(Tag);
impl<'de> DeserializeSeed<'de> for ValueTag {
    type Value = usize;
    fn deserialize<D: serde::Deserializer<'de>>(self, decoder: D) -> Result<usize, D::Error> {
        let raw = <&'de RawValue>::deserialize(decoder)?;
        require_shape(raw, b"\"")?;
        let mut tag = serde_json::Deserializer::from_str(raw.get());
        let value = self.0.deserialize(&mut tag).map_err(D::Error::custom)?;
        tag.end().map_err(D::Error::custom)?;
        Ok(value)
    }
}

pub(super) struct Unsigned<const MAX: u64>(pub(super) u64);
impl<'de, const MAX: u64> Deserialize<'de> for Unsigned<MAX> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        // Borrow and validate the complete JSON leaf before interpreting digits.
        // RawValue's ignore_integer path never invokes float_roundtrip/lexical.
        let raw = <&'de RawValue>::deserialize(decoder)?;
        let mut value = 0u64;
        for digit in raw.get().bytes() {
            if !digit.is_ascii_digit() {
                return Err(D::Error::custom(
                    "image-return unsigned decimal integer required",
                ));
            }
            value = value
                .checked_mul(10)
                .and_then(|value| value.checked_add(u64::from(digit - b'0')))
                .filter(|value| *value <= MAX)
                .ok_or_else(|| D::Error::custom("image-return integer exceeds bound"))?;
        }
        Ok(Unsigned(value))
    }
}

fn absent<E: Error, T>(value: &Option<T>) -> Result<(), E> {
    if value.is_none() {
        Ok(())
    } else {
        Err(E::custom("duplicate image-return field"))
    }
}
fn required<E: Error, T>(value: Option<T>) -> Result<T, E> {
    value.ok_or_else(|| E::custom("missing image-return field"))
}
fn end<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<(), A::Error> {
    if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
        Err(A::Error::custom("extra image-return sequence element"))
    } else {
        Ok(())
    }
}
struct Envelope<'a> {
    protocol: u16,
    token: &'a RawValue,
    command: &'a RawValue,
}
impl<'de> Deserialize<'de> for Envelope<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Parts;
        impl<'de> Visitor<'de> for Parts {
            type Value = Envelope<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an image-return envelope")
            }
            fn visit_str<E: Error>(self, _: &str) -> Result<Self::Value, E> {
                Err(E::custom("image-return envelope required"))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let (mut kind, mut protocol, mut token, mut command) = (None, None, None, None);
                while let Some(key) =
                    map.next_key_seed(Tag(&["kind", "protocol", "token", "command"]))?
                {
                    match key {
                        0 => {
                            absent(&kind)?;
                            kind =
                                Some(map.next_value_seed(ValueTag(Tag(&[
                                    "body-member-image-return",
                                ])))?);
                        }
                        1 => {
                            absent(&protocol)?;
                            protocol = Some(map.next_value::<Unsigned<65535>>()?.0 as u16);
                        }
                        2 => {
                            absent(&token)?;
                            token = Some(map.next_value()?);
                        }
                        3 => {
                            absent(&command)?;
                            command = Some(map.next_value()?);
                        }
                        _ => unreachable!(),
                    }
                }
                required(kind)?;
                Ok(Envelope {
                    protocol: required(protocol)?,
                    token: required(token)?,
                    command: required(command)?,
                })
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                required(seq.next_element_seed(ValueTag(Tag(&["body-member-image-return"])))?)?;
                let value = Envelope {
                    protocol: required(seq.next_element::<Unsigned<65535>>()?)?.0 as u16,
                    token: required(seq.next_element()?)?,
                    command: required(seq.next_element()?)?,
                };
                end(&mut seq)?;
                Ok(value)
            }
        }
        decoder.deserialize_any(Parts)
    }
}

enum Operation {
    Begin,
    Complete,
}
struct Command<'a> {
    operation: Operation,
    presentation: Option<&'a RawValue>,
    images: Option<&'a RawValue>,
    proof: Option<&'a RawValue>,
}
fn operation(index: usize) -> Operation {
    if index == 0 {
        Operation::Begin
    } else {
        Operation::Complete
    }
}
fn command(input: &str) -> Result<Command<'_>, serde_json::Error> {
    struct Parts;
    impl<'de> Visitor<'de> for Parts {
        type Value = Command<'de>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an image-return command")
        }
        fn visit_str<E: Error>(self, _: &str) -> Result<Self::Value, E> {
            Err(E::custom("image-return command required"))
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let (mut op, mut presentation, mut images, mut proof) = (None, None, None, None);
            while let Some(key) =
                map.next_key_seed(Tag(&["operation", "presentation", "images", "proof"]))?
            {
                match key {
                    0 => {
                        absent(&op)?;
                        op = Some(map.next_value_seed(ValueTag(Tag(&["begin", "complete"])))?);
                    }
                    1 => {
                        absent(&presentation)?;
                        presentation = Some(map.next_value()?);
                    }
                    2 => {
                        absent(&images)?;
                        images = Some(map.next_value()?);
                    }
                    3 => {
                        absent(&proof)?;
                        proof = Some(map.next_value()?);
                    }
                    _ => unreachable!(),
                }
            }
            Ok(Command {
                operation: operation(required(op)?),
                presentation,
                images,
                proof,
            })
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
            let operation = operation(required(
                seq.next_element_seed(ValueTag(Tag(&["begin", "complete"])))?,
            )?);
            let mut value = Command {
                operation,
                presentation: None,
                images: None,
                proof: None,
            };
            match value.operation {
                Operation::Begin => {
                    value.presentation = Some(required(seq.next_element()?)?);
                    value.images = Some(required(seq.next_element()?)?);
                }
                Operation::Complete => {
                    value.proof = Some(required(seq.next_element()?)?);
                }
            }
            end(&mut seq)?;
            Ok(value)
        }
    }
    let raw = validated_span(input, b"{[")?;
    let mut decoder = serde_json::Deserializer::from_str(raw.get());
    let value = serde::Deserializer::deserialize_any(&mut decoder, Parts)?;
    decoder.end()?;
    Ok(value)
}

pub(super) struct ImageBounds {
    pub(super) maximum_images: usize,
    pub(super) maximum_compressed_bytes: usize,
}
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct ImageLayout {
    pub(super) images: usize,
    pub(super) compressed_bytes: usize,
    pub(super) largest_image: usize,
}
pub(super) struct BorrowedEnvelope<'a> {
    pub(super) protocol: u16,
    pub(super) token: &'a RawValue,
    pub(super) token_bytes: usize,
    pub(super) command: BorrowedCommand<'a>,
}
pub(super) enum BorrowedCommand<'a> {
    Begin {
        presentation: &'a RawValue,
        images: &'a RawValue,
        layout: ImageLayout,
    },
    Complete {
        proof: &'a RawValue,
    },
}

/// Preparation only. This does not authenticate, validate nested presentation
/// semantics, construct a Request/Command, or grant workspace or readiness.
/// Production callers must hold actual ingress custody before even this scan.
pub(super) fn inspect<'a>(
    frame: &'a str,
    bounds: &ImageBounds,
) -> Result<BorrowedEnvelope<'a>, serde_json::Error> {
    let raw = validated_span(frame, b"{[")?;
    let envelope: Envelope<'_> = serde_json::from_str(raw.get())?;
    let token_bytes = images::inspect_token(envelope.token.get())?;
    let command = command(envelope.command.get())?;
    let invalid =
        || <serde_json::Error as Error>::custom("image command fields differ from operation");
    let command = match command.operation {
        Operation::Begin if command.proof.is_none() => {
            let presentation = command.presentation.ok_or_else(invalid)?;
            let images = command.images.ok_or_else(invalid)?;
            let layout = images::inspect(images.get(), bounds)?;
            BorrowedCommand::Begin {
                presentation,
                images,
                layout,
            }
        }
        Operation::Complete if command.presentation.is_none() && command.images.is_none() => {
            BorrowedCommand::Complete {
                proof: command.proof.ok_or_else(invalid)?,
            }
        }
        _ => return Err(invalid()),
    };
    Ok(BorrowedEnvelope {
        protocol: envelope.protocol,
        token: envelope.token,
        token_bytes,
        command,
    })
}

#[path = "image_return_ingress/images.rs"]
mod images;
#[cfg(feature = "member-return-prototype")]
#[path = "image_return_ingress/proof.rs"]
pub(super) mod proof;
#[cfg(test)]
#[path = "image_return_ingress/tests.rs"]
mod tests;
