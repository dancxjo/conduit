//! Prose from the authored page that owns the native Tour's canonical specimen.
use alloc::{format, string::String, vec::Vec};
use conduit_presentation::{
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

pub const TOUR_LESSON_SUBJECT: &str = "tour/lesson";
const CHAPTER: &str = include_str!("../../content/chapter-1.md");
const MAX_BLOCKS: usize = 24;

pub(crate) fn append(
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    text: &mut Vec<PresentationText>,
) -> Result<(), &'static str> {
    subjects.push(PresentationSubject {
        identity: TOUR_LESSON_SUBJECT.into(),
        role: PresentationRole::Document,
        accessibility_name: "Tour lesson".into(),
        label: "Tour lesson".into(),
    });
    relationships.push(PresentationRelationship {
        source: crate::TOUR_WORKSPACE_SUBJECT.into(),
        target: TOUR_LESSON_SUBJECT.into(),
        kind: PresentationRelationshipKind::Contains,
    });
    for (index, (heading, value)) in prose(CHAPTER)?.into_iter().enumerate() {
        let identity = format!("{TOUR_LESSON_SUBJECT}/{index}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Info,
            accessibility_name: if heading { "Heading" } else { "Paragraph" }.into(),
            label: if heading { "Heading" } else { "Paragraph" }.into(),
        });
        relationships.push(PresentationRelationship {
            source: TOUR_LESSON_SUBJECT.into(),
            target: identity.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        text.push(PresentationText {
            subject: identity,
            text: value,
        });
    }
    Ok(())
}

fn prose(markdown: &str) -> Result<Vec<(bool, String)>, &'static str> {
    let mut blocks = Vec::new();
    let mut metadata = false;
    let mut code = false;
    for (index, line) in markdown.lines().enumerate() {
        if line == "---" && (index == 0 || metadata) {
            metadata = !metadata;
            continue;
        }
        if metadata {
            continue;
        }
        if line.starts_with("```") {
            code = !code;
            continue;
        }
        if code || line.trim().is_empty() {
            continue;
        }
        let heading = line.starts_with('#');
        let value = line.trim_start_matches('#').trim().replace("**", "");
        if blocks.len() == MAX_BLOCKS
            || value.len() > conduit_presentation::MAX_PRESENTATION_TEXT_BYTES
        {
            return Err("tour-lesson-prose-capacity");
        }
        blocks.push((heading, value));
    }
    if metadata || code || blocks.is_empty() {
        return Err("tour-lesson-prose-malformed");
    }
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_page_prose_survives_projection_without_front_matter_or_source() {
        let blocks = prose(CHAPTER).unwrap();
        assert_eq!(blocks[0], (true, "One Program, Many Computers".into()));
        assert!(blocks[1].1.contains("one logical computer"));
        assert!(
            blocks
                .last()
                .unwrap()
                .1
                .ends_with("nearby Forms are unaffected.")
        );
        assert!(blocks.iter().all(|(_, text)| !text.contains("**")
            && !text.contains("stage:")
            && !text.contains("words:")));
        // Every prose line in the authored chapter has a corresponding block.
        assert_eq!(blocks.len(), 11);
    }
    #[test]
    fn malformed_or_over_capacity_content_is_refused() {
        assert!(prose("---\nmissing terminator").is_err());
        assert!(prose("# Title\n```conduit run\nmissing terminator").is_err());
        assert!(prose(&"paragraph\n".repeat(MAX_BLOCKS + 1)).is_err());
    }
}
