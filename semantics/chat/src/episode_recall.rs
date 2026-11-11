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
    let mut candidates = Vec::with_capacity(memories.len());
    for memory in memories {
        let score = |text: &str, identity: &str| shared_terms(query, text, identity);
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
        let outcome_terms = match memory.outcome() {
            crate::EpisodeOutcome::Produced(_) => "model attempt outcome produced",
            crate::EpisodeOutcome::ProviderLost(_) => {
                "model attempt outcome provider lost unavailable"
            }
        };
        let trigger_terms = score(memory.observation().content(), "").max(score(outcome_terms, ""));
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

fn project_historical_item(
    item: &serde_json::Value,
    asks_for_percentage: bool,
) -> serde_json::Value {
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

/// The existing bounded ASCII lexical law, usable over borrowed exact packets.
/// The query and candidate are already bounded by their owning contracts.
pub(crate) fn shared_terms(query: &str, text: &str, identity: &str) -> usize {
    keywords(query)
        .map(normalize_keyword)
        .enumerate()
        .filter(|(index, term)| {
            !keywords(query)
                .map(normalize_keyword)
                .take(*index)
                .any(|earlier| earlier.eq_ignore_ascii_case(term))
                && keywords(text)
                    .chain(keywords(identity))
                    .map(normalize_keyword)
                    .any(|word| word.eq_ignore_ascii_case(term))
        })
        .count()
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
pub(super) fn compact_evidence(evidence: &str, observation: &str) -> Option<alloc::string::String> {
    let snapshot: serde_json::Value = serde_json::from_str(evidence).ok()?;
    let compact_columns = snapshot.get("classification_fields").is_some_and(|fields| {
        [
            "domain",
            "origin",
            "availability",
            "certainty",
            "temporal_role",
        ]
        .iter()
        .enumerate()
        .all(|(index, field)| fields[index] == *field)
    });
    let items: Vec<_> = snapshot["evidence"]
        .as_array()?
        .iter()
        .filter_map(|item| {
            let content = if item["json_string_ref"] == "observation.content" {
                serde_json::to_string(observation).ok()?
            } else { item["utf8_content"].as_str()?.into() };
            Some(serde_json::json!({
                "identity": item["identity"],
                "domain": if compact_columns { &item["classifications"][0] } else { &item["classifications"]["domain"] },
                "availability": if compact_columns { &item["classifications"][2] } else { &item["classifications"]["availability"] },
                "certainty": if compact_columns { &item["classifications"][3] } else { &item["classifications"]["certainty"] },
                "origin": if compact_columns { &item["classifications"][1] } else { &item["classifications"]["origin"] },
                "temporal_role": if compact_columns { &item["classifications"][4] } else { &item["classifications"]["temporal_role"] },
                "utf8_content": content,
                "sources": item["sources"],
                "observed_at": item["observed_at"],
            }))
        })
        .collect();
    let mut compact = serde_json::json!({
        "revision": snapshot["revision"],
        "provenance_revision": snapshot["provenance_revision"],
        "evidence": items,
    });
    // A tuple's columns are part of its time provenance, not display metadata.
    if let Some(fields) = snapshot.get("time_fields") {
        compact["time_fields"] = fields.clone();
    }
    Some(alloc::string::ToString::to_string(&compact))
}

pub(super) fn asks_model_outcome(query: &str) -> bool {
    keywords(query)
        .map(normalize_keyword)
        .any(|term| term.eq_ignore_ascii_case("outcome"))
        && keywords(query)
            .map(normalize_keyword)
            .any(|term| term.eq_ignore_ascii_case("model") || term.eq_ignore_ascii_case("provider"))
}

pub(super) fn asks_about_past(query: &str) -> bool {
    query
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|word| {
            ["was", "were", "did", "earlier", "previous", "reported"]
                .iter()
                .any(|past| word.eq_ignore_ascii_case(past))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn retained_named_classifications_preserve_known_and_unknown_fields() {
        for classifications in [
            serde_json::json!({"domain":"human_utterance","origin":"observation","availability":"present","certainty":"certain","temporal_role":"current"}),
            serde_json::json!({"origin":"observation","temporal_role":"current"}),
        ] {
            let evidence = serde_json::json!({"evidence":[{"identity":"one","classifications":classifications,"utf8_content":"report","sources":[]}]}).to_string();
            let compact: serde_json::Value =
                serde_json::from_str(&compact_evidence(&evidence, "report").unwrap()).unwrap();
            for field in [
                "domain",
                "origin",
                "availability",
                "certainty",
                "temporal_role",
            ] {
                assert_eq!(compact["evidence"][0][field], classifications[field]);
            }
        }
    }

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
