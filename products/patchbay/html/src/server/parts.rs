//! Bounded HTML Parts commands over the transported canonical projection.

use super::{PatchbayHtmlServer, ServerError};
use patchbay_model::{PartsAction, PartsView};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HtmlPartsInteractionInput {
    presentation_id: String,
    body_id: String,
    action: String,
    target: String,
}

impl PatchbayHtmlServer {
    pub(super) fn apply_parts_interaction(&mut self, bytes: &[u8]) -> Result<Vec<u8>, ServerError> {
        let input: HtmlPartsInteractionInput =
            serde_json::from_slice(bytes).map_err(|_| ServerError::InvalidRequest)?;
        let parts = self
            .snapshot
            .parts
            .as_ref()
            .ok_or(ServerError::InvalidRequest)?;
        self.snapshot.interaction.revision = self.snapshot.interaction.revision.saturating_add(1);
        self.snapshot.interaction.selected_part = None;
        self.snapshot.interaction.selected_candidate = None;

        let stale = input.presentation_id != self.snapshot.presentation.identity.as_str()
            || input.body_id != parts.body_id.as_str();
        let mut disposition = "Refused";
        let feedback = if stale {
            "Parts action refused: the presentation or Body basis is stale".into()
        } else if input.action == "Inspect" {
            if parts
                .parts
                .iter()
                .any(|row| row.details.part_id.as_str() == input.target)
            {
                let subject = format!("part/{}", input.target);
                self.snapshot
                    .entrance
                    .select(&self.snapshot.presentation, &subject)
                    .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
                self.snapshot.interaction.selected_part = Some(input.target);
                disposition = "Succeeded";
                "Exact Part facts selected without changing Body membership".into()
            } else if parts
                .wants_to_join
                .iter()
                .any(|row| row.candidate_id.as_str() == input.target)
            {
                let subject = format!("candidate/{}", input.target);
                self.snapshot
                    .entrance
                    .select(&self.snapshot.presentation, &subject)
                    .map_err(|error| ServerError::Interaction(format!("{error:?}")))?;
                self.snapshot.interaction.selected_candidate = Some(input.target);
                disposition = "Succeeded";
                "Exact candidate facts selected without admitting it".into()
            } else {
                "Parts action refused: the selected row is stale or unknown".into()
            }
        } else if action_is_current(parts, &input.action, &input.target) {
            "Parts action refused nonfatally: this HTML delivery has no attached Body coordinator"
                .into()
        } else {
            "Parts action refused: it is unavailable on the current canonical row".into()
        };
        self.snapshot.interaction.parts_disposition = Some(disposition.into());
        self.snapshot.interaction.parts_feedback = Some(feedback);
        self.encoded_snapshot = self.snapshot.encode()?;
        Ok(self.encoded_snapshot.clone())
    }
}

fn action_is_current(parts: &PartsView, action: &str, target: &str) -> bool {
    let expected = match action {
        "Admit" => Some(PartsAction::Admit),
        "Refuse" => Some(PartsAction::Refuse),
        "Revoke" => Some(PartsAction::Revoke),
        "SpawnBrowserPart" => Some(PartsAction::SpawnBrowserPart),
        "Replan" => Some(PartsAction::Replan),
        _ => None,
    };
    expected.is_some_and(|expected| {
        parts
            .parts
            .iter()
            .any(|row| row.details.part_id.as_str() == target && row.actions.contains(&expected))
            || parts
                .wants_to_join
                .iter()
                .any(|row| row.candidate_id.as_str() == target && row.actions.contains(&expected))
            || (parts.body_id.as_str() == target && parts.actions.contains(&expected))
    })
}
