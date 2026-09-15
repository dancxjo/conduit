//! Prose from the authored page that owns the native Tour's canonical specimen.
use alloc::{format, string::String, vec::Vec};
use conduit_presentation::{
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
    PresentationText,
};

pub const TOUR_LESSON_SUBJECT: &str = "tour/lesson";
const CHAPTERS: [&str; crate::TOUR_CHAPTER_COUNT as usize] = [
    include_str!("../../content/chapter-1.md"),
    include_str!("../../content/chapter-2.md"),
    include_str!("../../content/chapter-3.md"),
    include_str!("../../content/chapter-4.md"),
    include_str!("../../content/chapter-5.md"),
    include_str!("../../content/chapter-6.md"),
    include_str!("../../content/chapter-8.md"),
];
const MAX_BLOCKS: usize = 24;

pub(crate) fn append(
    chapter: u8,
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    text: &mut Vec<PresentationText>,
) -> Result<(), &'static str> {
    let markdown = chapter_markdown(chapter)?;
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
    for (index, (heading, value)) in prose(markdown)?.into_iter().enumerate() {
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

pub(crate) fn chapter_title(chapter: u8) -> Result<String, &'static str> {
    prose(chapter_markdown(chapter)?)?
        .into_iter()
        .find_map(|(heading, text)| heading.then_some(text))
        .ok_or("tour-lesson-title-missing")
}

fn chapter_markdown(chapter: u8) -> Result<&'static str, &'static str> {
    CHAPTERS
        .get(usize::from(chapter))
        .copied()
        .ok_or("tour-lesson-chapter-refused")
}

pub(crate) fn stage_source(chapter: u8, stage: u8) -> Result<String, &'static str> {
    let markdown = chapter_markdown(chapter)?;
    let mut sources = Vec::new();
    let mut lines = markdown.lines();
    while let Some(line) = lines.next() {
        if !matches!(
            line,
            "```conduit run"
                | "```conduit run recursive"
                | "```conduit compare"
                | "```conduit run two-host"
                | "```conduit run two-host plan"
        ) {
            continue;
        }
        let mut source = String::new();
        let mut terminated = false;
        for line in lines.by_ref() {
            if line == "```" {
                terminated = true;
                break;
            }
            if !source.is_empty() {
                source.push('\n');
            }
            source.push_str(line);
        }
        if !terminated {
            return Err("tour-stage-source-malformed");
        }
        sources.push(source);
    }
    sources
        .get(usize::from(stage))
        .cloned()
        .ok_or("tour-stage-source-refused")
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
    fn every_authored_page_survives_projection_within_the_finite_bound() {
        let pages = CHAPTERS
            .iter()
            .map(|chapter| prose(chapter).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(pages.len(), crate::TOUR_CHAPTER_COUNT as usize);
        assert!(
            pages
                .iter()
                .all(|blocks| !blocks.is_empty() && blocks.len() <= MAX_BLOCKS)
        );
        let blocks = &pages[0];
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
        assert_eq!(
            chapter_title(1).unwrap(),
            "Faces, Backs, and implementation"
        );
        assert_eq!(chapter_title(6).unwrap(), "Birth, spores, and the Crèche");
        assert_eq!(chapter_title(7), Err("tour-lesson-chapter-refused"));
        let sources = (0..crate::TOUR_CHAPTER_COUNT)
            .flat_map(|chapter| {
                (0..crate::TOUR_CHAPTERS[usize::from(chapter)].stages.len() as u8)
                    .map(move |stage| stage_source(chapter, stage).unwrap())
            })
            .collect::<Vec<_>>();
        assert_eq!(sources.len(), 7);
        assert!(sources[0].contains("form meet-one-gear"));
        assert!(sources[3].contains("form same-morse-caller"));
        assert!(sources[6].contains("form hello-across"));
    }
    #[test]
    fn malformed_or_over_capacity_content_is_refused() {
        assert!(prose("---\nmissing terminator").is_err());
        assert!(prose("# Title\n```conduit run\nmissing terminator").is_err());
        assert!(prose(&"paragraph\n".repeat(MAX_BLOCKS + 1)).is_err());
    }
}
