use conduit_face::{FaceFragment, FaceProperty, FacePropertyValue, FaceRole};

/// A retained observation remains truthful within the same civil minute. Keep
/// its original timestamp and typed fact rather than rewriting its provenance.
pub(super) fn same_presented_minute(previous: &FaceFragment, next: &FaceFragment) -> bool {
    if previous.basis != next.basis
        || previous.subjects != next.subjects
        || previous.relationships != next.relationships
        || previous.composition != next.composition
        || previous.actions != next.actions
        || previous.disclosures != next.disclosures
        || previous.temporal_references != next.temporal_references
        || previous.temporal_facts != next.temporal_facts
        || previous.properties.len() != next.properties.len()
        || previous.text.len() != next.text.len()
    {
        return false;
    }
    let mut clocks = previous.subjects.iter().filter(
        |subject| matches!(&subject.role, FaceRole::Semantic(kind) if kind.as_str() == "time/civil-observation"),
    );
    let Some(clock) = clocks.next() else {
        return false;
    };
    if clocks.next().is_some() {
        return false;
    }
    let clock = clock.identity.as_str();
    let Some(old_time) = clock_text(previous, clock, "local-time") else {
        return false;
    };
    let Some(new_time) = clock_text(next, clock, "local-time") else {
        return false;
    };
    let (Some(old_minute), Some(new_minute)) = (civil_minute(old_time), civil_minute(new_time))
    else {
        return false;
    };
    if old_minute != new_minute || old_time > new_time {
        return false;
    }
    let weather_subject = |subject: &str| {
        previous.subjects.iter().any(|fact| fact.identity == subject &&
            matches!(&fact.role, FaceRole::Semantic(kind) if kind.as_str() == "weather/conditions"))
    };
    let properties_unchanged =
        previous
            .properties
            .iter()
            .zip(&next.properties)
            .all(|(old, new)| {
                if old.subject != new.subject || old.name != new.name {
                    return false;
                }
                match (
                    old.subject.as_str() == clock,
                    old.name.as_str(),
                    &old.value,
                    &new.value,
                ) {
                    (
                        true,
                        "local-time",
                        FacePropertyValue::Text(a),
                        FacePropertyValue::Text(b),
                    ) => a == old_time && b == new_time,
                    (
                        true,
                        "local-date-time",
                        FacePropertyValue::TypedValue { contract: a, .. },
                        FacePropertyValue::TypedValue { contract: b, .. },
                    ) => a == b,
                    (
                        true,
                        "observed-at-unix-ms",
                        FacePropertyValue::Count(a),
                        FacePropertyValue::Count(b),
                    ) => same_epoch_minute(*a, *b),
                    (
                        false,
                        "reference-at-unix-ms",
                        FacePropertyValue::Count(a),
                        FacePropertyValue::Count(b),
                    ) if weather_subject(&old.subject) => same_epoch_minute(*a, *b),
                    _ => old == new,
                }
            });
    properties_unchanged
        && previous.text.iter().zip(&next.text).all(|(old, new)| {
            if old == new {
                return true;
            }
            if old.subject != clock || new.subject != clock {
                return false;
            }
            let mut a = old.text.splitn(3, " · ");
            let mut b = new.text.splitn(3, " · ");
            a.next() == b.next()
                && a.next() == Some(old_time)
                && b.next() == Some(new_time)
                && a.next() == b.next()
        })
}

fn clock_text<'a>(fragment: &'a FaceFragment, subject: &str, property: &str) -> Option<&'a str> {
    let FaceProperty {
        value: FacePropertyValue::Text(text),
        ..
    } = fragment
        .properties
        .iter()
        .find(|item| item.subject == subject && item.name == property)?
    else {
        return None;
    };
    Some(text)
}

fn civil_minute(time: &str) -> Option<&str> {
    let bytes = time.as_bytes();
    if bytes.len() != 8
        || bytes[2] != b':'
        || bytes[5] != b':'
        || ![0, 1, 3, 4, 6, 7]
            .into_iter()
            .all(|n| bytes[n].is_ascii_digit())
    {
        return None;
    }
    time.get(..5)
}

fn same_epoch_minute(previous: u64, next: u64) -> bool {
    previous <= next && previous / 60_000 == next / 60_000
}

#[cfg(test)]
#[path = "ambient_cadence_tests.rs"]
mod tests;
