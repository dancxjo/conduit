//! Bounded lexical retrieval over retained episode observations.
use alloc::vec::Vec;

use crate::EpisodeMemory;

#[derive(Clone)]
pub(super) struct RecallCandidate<'a> {
    pub(super) memory: &'a EpisodeMemory,
    pub(super) shared_terms: Option<usize>,
    pub(super) trigger_terms: usize,
    pub(super) historical_evidence: Option<serde_json::Value>,
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
                trigger_terms: 0,
                historical_evidence: None,
            })
            .collect();
    }
    let mut terms = Vec::new();
    for term in keywords(query).map(normalize_keyword) {
        if !terms
            .iter()
            .any(|previous: &&str| previous.eq_ignore_ascii_case(term))
        {
            terms.push(term);
        }
    }
    let mut candidates = Vec::with_capacity(memories.len());
    for memory in memories {
        let score = |text: &str, identity: &str| {
            terms
                .iter()
                .filter(|term| {
                    keywords(text)
                        .chain(keywords(identity))
                        .map(normalize_keyword)
                        .any(|word| word.eq_ignore_ascii_case(term))
                })
                .count()
        };
        let evidence = memory
            .evidence()
            .as_deref()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
        let best_item = evidence.as_ref().and_then(|snapshot| {
            snapshot["evidence"]
                .as_array()?
                .iter()
                .filter(|item| {
                    matches!(
                        item["origin"].as_str(),
                        Some("observation" | "human_reported" | "deterministic_derived")
                    )
                })
                .filter_map(|item| {
                    let score = score(
                        item["utf8_content"].as_str()?,
                        item["identity"].as_str().unwrap_or(""),
                    );
                    (score > 0).then_some((score, item))
                })
                .max_by_key(|(score, _)| *score)
                .map(|(item_score, item)| {
                    let item = project_historical_item(item, score("percent", "") > 0);
                    (
                        item_score,
                        serde_json::json!({
                            "revision": snapshot["revision"],
                            "provenance_revision": snapshot["provenance_revision"],
                            "evidence": [item],
                        }),
                    )
                })
        });
        let trigger_terms = score(memory.observation().content(), "");
        let shared_terms = trigger_terms.max(best_item.as_ref().map_or(0, |(score, _)| *score));
        if shared_terms > 0 {
            candidates.push(RecallCandidate {
                memory,
                shared_terms: Some(shared_terms),
                trigger_terms,
                historical_evidence: best_item.map(|(_, evidence)| evidence),
            });
        }
    }
    // The composer drops the lowest-ranked candidate first under prompt pressure.
    candidates
        .sort_unstable_by_key(|candidate| (candidate.shared_terms, *candidate.memory.episode()));
    candidates
}

fn project_historical_item(item: &serde_json::Value, asks_for_percentage: bool) -> serde_json::Value {
    let mut projected = item.clone();
    if let Some(fields) = projected.as_object_mut() {
        // Other readings can depend on coupled fields such as value, unit,
        // uncertainty, or calibration. Keep their content together.
        if asks_for_percentage
            && item["identity"] == "pete/homeostasis"
            && item["origin"] == "deterministic_derived"
        {
            let charge = item["utf8_content"]
                .as_str()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
                .and_then(|body| {
                    body["charge_percent"]
                        .as_str()
                        .map(alloc::string::String::from)
                });
            if let Some(charge) = charge {
                let reading = serde_json::json!({ "charge_percent": charge });
                fields.insert(
                    "utf8_content".into(),
                    serde_json::Value::String(alloc::string::ToString::to_string(&reading)),
                );
            }
        }
        fields.remove("observed_at");
    }
    projected
}

fn normalize_keyword(word: &str) -> &str {
    if word.eq_ignore_ascii_case("percentage") {
        "percent"
    } else {
        word
    }
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

/// Keep the evidence and its source context, dropping rendering fields that
/// are unnecessary for a historical model turn.
pub(super) fn compact_evidence(evidence: &str) -> Option<alloc::string::String> {
    let snapshot: serde_json::Value = serde_json::from_str(evidence).ok()?;
    let items: Vec<_> = snapshot["evidence"]
        .as_array()?
        .iter()
        .filter_map(|item| {
            Some(serde_json::json!({
                "identity": item["identity"],
                "origin": item["classifications"]["origin"],
                "temporal_role": item["classifications"]["temporal_role"],
                "utf8_content": item["utf8_content"].as_str()?,
                "sources": item["sources"],
                "observed_at": item["observed_at"],
            }))
        })
        .collect();
    Some(alloc::string::ToString::to_string(&serde_json::json!({
        "revision": snapshot["revision"],
        "provenance_revision": snapshot["provenance_revision"],
        "evidence": items,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episode_loop_recall_keeps_coupled_measurement_fields() {
        let content = r#"{"voltage":13,"unit":"V","uncertainty":0.1,"calibration":"meter/2"}"#;
        let item = serde_json::json!({
            "identity": "pete/battery/4",
            "origin": "observation",
            "utf8_content": content,
            "sources": [{"sign":"battery/4"}],
        });
        let recalled = project_historical_item(&item, true);
        assert_eq!(recalled["utf8_content"], content);
        assert_eq!(recalled["sources"], item["sources"]);
    }
}
