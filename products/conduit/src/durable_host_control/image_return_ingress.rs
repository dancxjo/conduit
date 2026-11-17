//! Borrowed image-return preparation, fenced from installed decoding until its
//! original-counter ingress workspace exists. Raw spans confer no authority.
use serde::{de::Error, Deserialize};
use serde_json::value::RawValue;

#[derive(Deserialize)]
#[serde(field_identifier)]
enum Kind {
    #[serde(rename = "body-member-image-return")]
    ImageReturn,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope<'a> {
    kind: Kind,
    protocol: u16,
    #[serde(borrow)]
    token: &'a RawValue,
    #[serde(borrow)]
    command: &'a RawValue,
}
#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "kebab-case")]
enum Operation {
    Begin,
    Complete,
}
// Unlike Option<&RawValue>, this distinguishes a missing field from JSON null.
// A null field belonging to the other variant must still be refused.
#[derive(Default)]
struct Field<'a>(Option<&'a RawValue>);
impl<'de: 'a, 'a> Deserialize<'de> for Field<'a> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        <&'de RawValue>::deserialize(decoder).map(|value| Self(Some(value)))
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Command<'a> {
    operation: Operation,
    #[serde(default, borrow)]
    presentation: Field<'a>,
    #[serde(default, borrow)]
    images: Field<'a>,
    #[serde(default, borrow)]
    proof: Field<'a>,
}

fn command(input: &str) -> Result<Command<'_>, serde_json::Error> {
    struct Parts;
    impl<'de> serde::de::Visitor<'de> for Parts {
        type Value = Command<'de>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an image-return command")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
            Command::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> Result<Self::Value, A::Error> {
            let operation: Operation = seq
                .next_element()?
                .ok_or_else(|| A::Error::custom("missing operation"))?;
            let mut value = Command {
                operation,
                presentation: Field::default(),
                images: Field::default(),
                proof: Field::default(),
            };
            match value.operation {
                Operation::Begin => {
                    value.presentation = seq
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("missing presentation"))?;
                    value.images = seq
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("missing images"))?;
                }
                Operation::Complete => {
                    value.proof = seq
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("missing proof"))?
                }
            }
            if seq.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(A::Error::custom("extra image command element"));
            }
            Ok(value)
        }
    }
    let mut decoder = serde_json::Deserializer::from_str(input);
    let command = serde::Deserializer::deserialize_any(&mut decoder, Parts)?;
    decoder.end()?;
    Ok(command)
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
    let envelope: Envelope<'_> = serde_json::from_str(frame)?;
    let Kind::ImageReturn = envelope.kind;
    let token_bytes = images::inspect_token(envelope.token.get())?;
    let command = command(envelope.command.get())?;
    let invalid =
        || <serde_json::Error as Error>::custom("image command fields differ from operation");
    let command = match command.operation {
        Operation::Begin if command.proof.0.is_none() => {
            let presentation = command.presentation.0.ok_or_else(invalid)?;
            let images = command.images.0.ok_or_else(invalid)?;
            let layout = images::inspect(images.get(), bounds)?;
            BorrowedCommand::Begin {
                presentation,
                images,
                layout,
            }
        }
        Operation::Complete if command.presentation.0.is_none() && command.images.0.is_none() => {
            BorrowedCommand::Complete {
                proof: command.proof.0.ok_or_else(invalid)?,
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
#[cfg(test)]
#[path = "image_return_ingress/tests.rs"]
mod tests;
