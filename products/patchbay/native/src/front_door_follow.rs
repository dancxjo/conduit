//! Native selection and rendering of exact portable FOLLOW correlations.

use conduit_presentation::{
    NavigationFollow, NavigationObservation, Presentation, PresentationCursor,
    PresentationNavigation,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NativeFollowRefusal {
    Unavailable,
    Ambiguous,
}

pub(super) fn exact_current_follow<'a>(
    navigation: &'a PresentationNavigation,
    cursor: &PresentationCursor,
    selected: Option<&str>,
) -> Result<&'a NavigationFollow, NativeFollowRefusal> {
    let Some(focus) = cursor.focus.as_deref() else {
        return Err(NativeFollowRefusal::Unavailable);
    };
    let candidates = navigation
        .follows
        .iter()
        .filter(|follow| follow.source_subject == focus)
        .collect::<Vec<_>>();
    match candidates.as_slice() {
        [] => Err(NativeFollowRefusal::Unavailable),
        [only] => Ok(only),
        _ => selected
            .and_then(|identity| {
                candidates
                    .into_iter()
                    .find(|follow| follow.identity == identity)
            })
            .ok_or(NativeFollowRefusal::Ambiguous),
    }
}

pub(super) fn append_follow_lines(
    presentation: &Presentation,
    observation: &NavigationObservation,
    selected_follow: Option<&str>,
    lines: &mut Vec<String>,
) -> Result<(), String> {
    for follow in &observation.current_follows {
        let destination = presentation
            .subjects
            .iter()
            .find(|subject| subject.identity == follow.target_subject)
            .ok_or_else(|| "FOLLOW destination subject is absent".to_string())?;
        let selected = observation.current_follows.len() == 1
            || selected_follow.is_some_and(|identity| identity == follow.identity);
        let binding = if selected {
            "F3 SELECTED"
        } else {
            "SHIFT-F3 CHOOSE"
        };
        lines.push(format!(
            "FOLLOW {:?} id={} TO {:?}: {}  ·  {:?}/{:?}  ·  [{binding}]",
            follow.relationship,
            follow.identity,
            destination.role,
            destination.accessibility_name,
            follow.target_place,
            follow.target_aspect
        ));
    }
    Ok(())
}
