//! Render declared Port roles/types and exact Cord endpoints from this Face.
use alloc::{format, string::String};
use conduit_presentation::{
    Presentation, PresentationPropertyValue, PresentationRelationshipKind, PresentationRole,
    PresentationSubject,
};

fn property<'a>(
    face: &'a Presentation,
    subject: &str,
    name: &str,
    identity: bool,
) -> Option<&'a str> {
    face.properties.iter().find_map(|p| {
        if p.subject != subject || p.name != name {
            return None;
        }
        match (&p.value, identity) {
            (PresentationPropertyValue::Identity(value), true)
            | (PresentationPropertyValue::Text(value), false) => Some(value.as_str()),
            _ => None,
        }
    })
}

pub(super) fn label(face: &Presentation, port: &PresentationSubject) -> String {
    let direction = match property(face, &port.identity, "direction", false) {
        Some("receiving") => "IN",
        Some("outgoing") => "OUT",
        _ => "PORT",
    };
    let kind = property(face, &port.identity, "value-kind", true).unwrap_or("type unavailable");
    format!("{direction} {kind} · {}", port.name)
}

fn endpoint<'a>(
    face: &'a Presentation,
    cord: &PresentationSubject,
    name: &str,
    direction: &str,
) -> Option<&'a PresentationSubject> {
    let semantic = property(face, &cord.identity, name, true)?;
    resolve(face, &cord.identity, semantic, direction)
}

pub(super) fn resolve<'a>(
    face: &'a Presentation,
    cord: &str,
    semantic: &str,
    direction: &str,
) -> Option<&'a PresentationSubject> {
    let mut matching = face.subjects.iter().filter(|port| {
        port.role == PresentationRole::Port
            && property(face, &port.identity, "semantic-id", true) == Some(semantic)
            && property(face, &port.identity, "direction", false) == Some(direction)
            && face.relationships.iter().any(|r| {
                r.kind == PresentationRelationshipKind::Connects
                    && r.source == cord
                    && r.target == port.identity
            })
    });
    let port = matching.next()?;
    matching.next().is_none().then_some(port)
}

pub(super) fn cord_summary(face: &Presentation, cord: &PresentationSubject) -> String {
    match (
        endpoint(face, cord, "source-port", "outgoing"),
        endpoint(face, cord, "sink-port", "receiving"),
    ) {
        (Some(source), Some(sink)) => format!(
            "Source OUT {} -> Sink IN {}",
            source.identity, sink.identity
        ),
        _ => "Cord endpoints not uniquely resolved; inspect the declared facts below".into(),
    }
}

#[cfg(test)]
#[path = "ports/tests.rs"]
mod tests;
