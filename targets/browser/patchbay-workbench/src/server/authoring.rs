//! Read-only queries: neither catalog inspection nor layout admission edits a Plot.

use super::configuration_input::ConfigurationInput;
use super::{http::write_response, PatchbayHtmlServer, ServerError};
use conduit_core::ExpandedPlotId;
use serde::Deserialize;
use std::net::TcpStream;

#[derive(Deserialize)]
#[serde(tag = "query", rename_all = "kebab-case", deny_unknown_fields)]
enum AuthoringQuery {
    Connections {
        revision: u64,
        expanded_plot_id: ExpandedPlotId,
        source: String,
    },
    Configuration {
        revision: u64,
        expanded_plot_id: ExpandedPlotId,
        gear: String,
        key: String,
        value: ConfigurationInput,
    },
}

impl PatchbayHtmlServer {
    pub(super) fn deliver_authoring_route(
        &self,
        first: &str,
        stream: &mut TcpStream,
        body: &[u8],
    ) -> Option<Result<(), ServerError>> {
        if !matches!(
            first,
            "POST /api/authoring-query HTTP/1.1" | "POST /api/workspace HTTP/1.1"
        ) {
            return None;
        }
        let result = if first == "POST /api/workspace HTTP/1.1" {
            Self::validate_workspace(body)
        } else {
            self.authoring_query(body)
        };
        let (status, value) = match result {
            Ok(value) => ("200 OK", value),
            Err(error) => (
                "400 Bad Request",
                serde_json::json!({ "diagnostic": error }),
            ),
        };
        Some(match serde_json::to_vec(&value) {
            Ok(bytes) => write_response(stream, status, "application/json; charset=utf-8", &bytes),
            Err(error) => Err(ServerError::Interaction(error.to_string())),
        })
    }

    fn validate_workspace(body: &[u8]) -> Result<serde_json::Value, String> {
        if body.len() > patchbay_application::MAX_WORKSPACE_BYTES {
            return Err("BoundExceeded".into());
        }
        let document: patchbay_application::PatchbayWorkspace =
            serde_json::from_slice(body).map_err(|error| error.to_string())?;
        document.validate().map_err(|error| format!("{error:?}"))?;
        Ok(serde_json::json!({ "valid": true }))
    }

    fn authoring_query(&self, body: &[u8]) -> Result<serde_json::Value, String> {
        let query: AuthoringQuery =
            serde_json::from_slice(body).map_err(|error| error.to_string())?;
        let session = self
            .zero_body_front_door
            .as_ref()
            .ok_or("authoring is not open")?;
        let editor = session
            .lock()
            .map_err(|_| "authoring session unavailable")?
            .opened_plot_editor()
            .ok_or("no checked Plot is open")?;
        match query {
            AuthoringQuery::Connections {
                revision,
                expanded_plot_id,
                source,
            } => {
                let candidates = editor
                    .authoring_connections(revision, &expanded_plot_id, &source)
                    .map_err(|error| error.to_string())?;
                Ok(
                    serde_json::json!({ "revision": revision, "source": source, "candidates": candidates }),
                )
            }
            AuthoringQuery::Configuration {
                revision,
                expanded_plot_id,
                gear,
                key,
                value,
            } => {
                let result = editor.validate_authoring_configuration(
                    revision,
                    &expanded_plot_id,
                    &gear,
                    &key,
                    value.checked()?,
                );
                Ok(serde_json::json!({
                    "valid": result.is_ok(),
                    "diagnostic": result.err().map(|error| error.to_string()),
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_endpoint_refuses_unknown_runtime_fields_future_versions_and_large_input() {
        let fixture = include_str!("../../../../../proof/browser/fixtures/patchbay-workspace.json");
        assert!(PatchbayHtmlServer::validate_workspace(fixture.as_bytes()).is_ok());
        let mut value: serde_json::Value = serde_json::from_str(fixture).unwrap();
        value["plan_id"] = "invented-plan".into();
        assert!(
            PatchbayHtmlServer::validate_workspace(&serde_json::to_vec(&value).unwrap()).is_err()
        );
        value.as_object_mut().unwrap().remove("plan_id");
        value["schema"] = "conduit.patchbay.workspace/v999".into();
        assert!(
            PatchbayHtmlServer::validate_workspace(&serde_json::to_vec(&value).unwrap()).is_err()
        );
        assert!(PatchbayHtmlServer::validate_workspace(&vec![
            b' ';
            patchbay_application::MAX_WORKSPACE_BYTES
                + 1
        ])
        .is_err());
    }
}
