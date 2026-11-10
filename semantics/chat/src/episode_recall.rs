//! Bounded lexical retrieval over retained episode observations.
use alloc::vec::Vec;

use crate::EpisodeMemory;

#[derive(Clone, Copy)]
pub(super) struct RecallCandidate<'a> {
    pub(super) memory: &'a EpisodeMemory,
    pub(super) shared_terms: Option<usize>,
}

pub(super) fn select<'a>(
    query: &str,
    memories: &'a [EpisodeMemory],
    lexical: bool,
) -> Vec<RecallCandidate<'a>> {
    if !lexical {
        return memories
            .iter()
            .map(|memory| RecallCandidate {
                memory,
                shared_terms: None,
            })
            .collect();
    }
    let mut terms = Vec::new();
    for term in keywords(query) {
        if !terms
            .iter()
            .any(|previous: &&str| previous.eq_ignore_ascii_case(term))
        {
            terms.push(term);
        }
    }
    let mut candidates = Vec::with_capacity(memories.len());
    for memory in memories {
        let shared_terms = terms
            .iter()
            .filter(|term| {
                keywords(memory.observation().content())
                    .any(|observed| observed.eq_ignore_ascii_case(term))
            })
            .count();
        if shared_terms > 0 {
            candidates.push(RecallCandidate {
                memory,
                shared_terms: Some(shared_terms),
            });
        }
    }
    // The composer drops the lowest-ranked candidate first under prompt pressure.
    candidates
        .sort_unstable_by_key(|candidate| (candidate.shared_terms, *candidate.memory.episode()));
    candidates
}

fn keywords(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|term| {
            term.len() >= 4
                && ![
                    "that", "this", "what", "when", "where", "which", "with", "would", "could",
                    "have", "from", "your", "about",
                ]
                .iter()
                .any(|common| common.eq_ignore_ascii_case(term))
        })
}
