//! Literal subject bindings for the reviewed clock/weather component Source.
//! Producing Source establishes neither a checked recipe nor a selected Mask.
use alloc::string::String;

/// Application component, interpreted by the ordinary checked layout Mask.
pub const AMBIENT_CLOCK_MASK_SOURCE: &str =
    include_str!("../components/ambient-clock-mask.conduit");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientClockComponentRefusal {
    SubjectBound,
    DuplicateSubject,
    Encoding,
    UnsupportedEncounter,
}

/// Explicit encounter policy, not a Host offer or permission to realize it.
/// The first contextual profile has two disjoint candidates, so declaration
/// order, predicate counts and viewport guesses cannot resolve selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmbientClockEncounter {
    Tile,
    Fullscreen,
}

impl TryFrom<&str> for AmbientClockEncounter {
    type Error = AmbientClockComponentRefusal;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "tile" => Ok(Self::Tile),
            "fullscreen" => Ok(Self::Fullscreen),
            _ => Err(AmbientClockComponentRefusal::UnsupportedEncounter),
        }
    }
}

impl AmbientClockEncounter {
    /// Exact reviewed Source root. The caller must independently use current
    /// observed Host offers, user constraints and ordinary Plan admission.
    pub const fn root(self) -> &'static str {
        match self {
            Self::Tile => "ambient-clock-tile",
            Self::Fullscreen => "ambient-clock-mask",
        }
    }
}

/// Bind explicit original Face references into the reviewed literal recipe.
/// The resulting document has its own Source identity and must pass ordinary
/// checking, planning and bound-slot validation against the actual Face.
/// Names, semantic facts, actions and output authority are never supplied here.
pub fn ambient_clock_mask_source(
    clock_subject: &str,
    weather_subject: &str,
    image_subject: &str,
) -> Result<String, AmbientClockComponentRefusal> {
    let subjects = [clock_subject, weather_subject, image_subject];
    if subjects
        .iter()
        .any(|subject| subject.is_empty() || subject.len() > 256)
    {
        return Err(AmbientClockComponentRefusal::SubjectBound);
    }
    if subjects
        .iter()
        .enumerate()
        .any(|(index, subject)| subjects[..index].contains(subject))
    {
        return Err(AmbientClockComponentRefusal::DuplicateSubject);
    }
    let slots = [
        "\"clock/date-time\"",
        "\"clock/weather\"",
        "\"wallpaper/image\"",
    ];
    if slots
        .iter()
        .zip([2, 1, 1])
        .any(|(slot, count)| AMBIENT_CLOCK_MASK_SOURCE.matches(slot).count() != count)
    {
        return Err(AmbientClockComponentRefusal::Encoding);
    }
    // Apply replacements in one scan so one supplied identity cannot be
    // interpreted as another template slot or as authored syntax.
    let replacements = subjects.map(|subject| {
        serde_json::to_string(subject).map_err(|_| AmbientClockComponentRefusal::Encoding)
    });
    let mut source = String::new();
    let mut remaining = AMBIENT_CLOCK_MASK_SOURCE;
    while !remaining.is_empty() {
        let next = slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                remaining
                    .find(slot)
                    .map(|offset| (offset, index, slot.len()))
            })
            .min_by_key(|(offset, _, _)| *offset);
        let Some((offset, index, length)) = next else {
            source.push_str(remaining);
            break;
        };
        source.push_str(&remaining[..offset]);
        source.push_str(replacements[index].as_ref().map_err(|error| *error)?);
        remaining = &remaining[offset + length..];
    }
    Ok(source)
}
