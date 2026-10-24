//! Lossless continuation of exact Sign facts within portable Face field bounds.
use super::{identity, text, ContentBuilder, PlayDocument};
use conduit_face::MAX_FACE_TEXT_BYTES;

pub(super) fn append_play(
    content: &mut ContentBuilder,
    subject: &str,
    play: Option<&PlayDocument>,
    placement: Option<&str>,
    connection: Option<&str>,
) {
    let Some(play) = play else { return };
    identity(
        content,
        subject,
        "active-play-id",
        play.active_play_id.as_str(),
    );
    text(
        content,
        subject,
        "play-state",
        &format!("{:?}", play.terminal),
    );
    text(content, subject, "pressure", "not exposed by this play");
    for (index, sign) in play
        .signs
        .iter()
        .filter(|sign| {
            placement.is_some_and(|value| {
                sign.placement_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == value)
            }) || connection.is_some_and(|value| {
                sign.connection_id
                    .as_ref()
                    .is_some_and(|id| id.as_str() == value)
            })
        })
        .enumerate()
    {
        append_sign_text(
            content,
            subject,
            &format!("sign-{index}"),
            &format!("{} · {:?}", sign.sign_id.as_str(), sign.kind),
        );
    }
}

fn append_sign_text(content: &mut ContentBuilder, subject: &str, name: &str, value: &str) {
    let mut rest = value;
    let mut part = 0;
    while !rest.is_empty() {
        let mut end = rest.len().min(MAX_FACE_TEXT_BYTES);
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        let key = if part == 0 {
            name.to_owned()
        } else {
            format!("{name}-continued-{part}")
        };
        text(content, subject, &key, &rest[..end]);
        rest = &rest[end..];
        part += 1;
    }
    if part > 1 {
        text(
            content,
            subject,
            &format!("{name}-parts"),
            &part.to_string(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_face::FacePropertyValue;

    #[test]
    fn sign_continuation_preserves_every_byte_and_scalar_at_field_boundaries() {
        for value in [
            "short Sign".to_owned(),
            "x".repeat(1024),
            format!("{}🪞{}", "x".repeat(1023), "Δοκιμή 世界".repeat(150)),
        ] {
            let mut content = ContentBuilder::new();
            append_sign_text(&mut content, "fixture/gear", "sign-0", &value);
            let mut restored = String::new();
            for property in &content.properties {
                let FacePropertyValue::Text(text) = &property.value else {
                    panic!("text fact")
                };
                assert!(!text.is_empty());
                assert!(text.len() <= MAX_FACE_TEXT_BYTES);
                if property.name != "sign-0-parts" {
                    restored.push_str(text);
                }
            }
            assert_eq!(restored, value);
            assert_eq!(content.properties[0].name, "sign-0");
            if value.len() <= MAX_FACE_TEXT_BYTES {
                assert_eq!(
                    content.properties.len(),
                    1,
                    "short facts keep their original wire shape"
                );
            } else {
                let parts = content.properties.len() - 1;
                assert_eq!(
                    content.properties.last().unwrap().value,
                    FacePropertyValue::Text(parts.to_string())
                );
                for (index, property) in content
                    .properties
                    .iter()
                    .skip(1)
                    .take(parts - 1)
                    .enumerate()
                {
                    assert_eq!(property.name, format!("sign-0-continued-{}", index + 1));
                }
            }
        }
    }
}
