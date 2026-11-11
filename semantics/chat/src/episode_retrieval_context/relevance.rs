//! Borrowed adapter for the existing episode lexical retrieval component.
use super::*;
use crate::episode_loop::shared_terms;
use crate::episode_retrieval_projection::raw_items;
impl PreparedEpisodeRetrievalContext {
    pub(super) fn rank_shared_terms(
        &mut self,
        snapshot: &BorrowedEpisodeRetrievalSnapshot<'_>,
        keys: &[EpisodeRetrievalCandidateKey],
    ) -> Result<(), R> {
        let experience = validate_canonical_structured_value(snapshot.input().experience)
            .map_err(|_| R::Malformed)?;
        let observation = field(experience, "observation")?;
        let query = core::str::from_utf8(leaf(field(observation, "content")?)?)
            .map_err(|_| R::Malformed)?;
        let mut kept = 0;
        for index in 0..self.group_count {
            let mut group = self.groups[index];
            let record = snapshot
                .original_record(group.episode)
                .map_err(|error| R::Window(EpisodeRetrievalWindowRefusal::Snapshot(error)))?;
            let evidence = field(record, "evidence")?
                .variant_payload("some")
                .map_err(|_| R::Malformed)?
                .ok_or(R::Malformed)?;
            let raw = leaf(evidence)?;
            let ranges = raw_items::locate_validated_items(raw).map_err(R::Projection)?;
            let mut best = (0, group.indices[0]);
            for key_index in &group.indices[..group.len] {
                let (start, end) = ranges[keys[*key_index].item as usize];
                let item = &raw[start..end];
                let content = decode_text(
                    raw_items::text_field(item, "utf8_content").map_err(R::Projection)?,
                )?;
                let identity =
                    decode_text(raw_items::text_field(item, "identity").map_err(R::Projection)?)?;
                // A retained question supplies its provenance, not an answer to
                // that same question. Factual reports rank ahead of questions.
                let score = if content
                    .as_str()
                    .trim_end_matches(['\"', '\\', ' '])
                    .ends_with('?')
                {
                    0
                } else {
                    shared_terms(query, content.as_str(), identity.as_str())
                };
                if score >= best.0 {
                    best = (score, *key_index);
                }
            }
            let outcome = field(record, "outcome")?;
            let outcome_terms = if outcome
                .variant_payload("provider_lost")
                .map_err(|_| R::Malformed)?
                .is_some()
            {
                "model attempt outcome provider lost unavailable"
            } else {
                "model attempt outcome produced"
            };
            let outcome_score = shared_terms(query, outcome_terms, "");
            let score = best.0.max(outcome_score);
            if score == 0 {
                self.irrelevant_episodes[self.irrelevant_len] = group.episode;
                self.irrelevant_len += 1;
                self.omitted_items += group.len;
                continue;
            }
            self.omitted_items += group.len - 1;
            group.len = 1;
            group.indices[0] = best.1;
            group.representative = best.1;
            self.groups[kept] = group;
            self.relevance_scores[kept] = score;
            kept += 1;
        }
        self.group_count = kept;
        Ok(())
    }
}

fn decode_text(raw: &str) -> Result<crate::FixedEvidenceText<4096>, R> {
    if !raw.starts_with('"') {
        return Ok(crate::FixedEvidenceText::default());
    }
    // Original JSON has already passed Web admission. Adapt only the working
    // spelling for serde-json-core; custody and rendering retain original bytes.
    let mut adapted = [0; 4096];
    let length = crate::episode_retrieval_evidence::surrogate_representation::adapt_validated_surrogate_pairs(raw.as_bytes(), &mut adapted);
    let mut scratch = [0; 4096];
    serde_json_core::from_slice_escaped(&adapted[..length], &mut scratch)
        .map(|(value, _)| value)
        .map_err(|_| R::Malformed)
}
