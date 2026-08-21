//! Native ordinary manifestation of portable temporal Presentation facts.

use conduit_presentation::{format_relative_time, Presentation, PresentationSubject};

pub(super) fn append_subject_age_lines(
    presentation: &Presentation,
    subject: &PresentationSubject,
    lines: &mut Vec<String>,
) {
    lines.extend(
        presentation
            .temporal_facts
            .iter()
            .filter(|fact| fact.subject == subject.identity)
            .map(|fact| {
                format!(
                    "  {}  ·  {:?}  ·  exact time in F2",
                    format_relative_time(fact),
                    fact.role
                )
            }),
    );
}
