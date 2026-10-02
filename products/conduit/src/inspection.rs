//! Schema-driven rendering for retained product truth.

use conduit_observatory::{build_report, render_text_report};
use std::path::Path;

const MAX_ARTIFACT_BYTES: usize = 8 * 1024 * 1024;
const BODY_BIOGRAPHY_SCHEMA: &str = "conduit.body/biography-evidence@2";

pub(crate) fn inspect(path: &Path) -> Result<String, String> {
    if path.extension().and_then(std::ffi::OsStr::to_str) == Some("conduit") {
        return inspect_plot(path);
    }
    let bytes = std::fs::read(path)
        .map_err(|error| format!("read inspection artifact {}: {error}", path.display()))?;
    if bytes.len() > MAX_ARTIFACT_BYTES {
        return Err("inspection artifact exceeds the 8 MiB product bound".into());
    }
    inspect_json(path, &bytes)
}

fn inspect_plot(path: &Path) -> Result<String, String> {
    let plot = crate::plot_source::load(path)?.expand_entry()?;
    Ok(format!(
        "Plot {}\nsource {}\nchecked {}\nexpanded {}\ngears {}\nconnections {}\nshared pools {}\n",
        plot.name,
        plot.source_document_id.as_str(),
        plot.checked_plot_id.as_str(),
        plot.expanded_plot_id.as_str(),
        plot.gears.len(),
        plot.connections.len(),
        plot.shared_pools.len(),
    ))
}

fn inspect_json(path: &Path, bytes: &[u8]) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode inspection artifact {}: {error}", path.display()))?;
    let schema = value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .ok_or("inspection artifact does not identify its schema")?;
    match schema {
        conduit_observatory::SNAPSHOT_SCHEMA => inspect_report(bytes),
        conduit_observatory::PLAN_ARTIFACT_SCHEMA => inspect_plan(bytes),
        conduit_observatory::PLAY_ARTIFACT_SCHEMA => inspect_play(bytes),
        conduit_observatory::SIGN_ARTIFACT_SCHEMA => inspect_sign(bytes),
        BODY_BIOGRAPHY_SCHEMA => inspect_body(bytes),
        crate::durable_host::INSTALL_SCHEMA => crate::durable_host::inspect_installation(path),
        _ => Err(format!("inspection does not yet support schema {schema}")),
    }
}

fn inspect_plan(bytes: &[u8]) -> Result<String, String> {
    let artifact: conduit_observatory::PlanArtifact =
        serde_json::from_slice(bytes).map_err(|error| format!("decode Plan artifact: {error}"))?;
    conduit_observatory::validate_plan_artifact(&artifact)?;
    let placement_count = artifact
        .plan
        .fragments
        .iter()
        .map(|fragment| fragment.placements.len())
        .sum::<usize>();
    let connection_count = artifact
        .plan
        .fragments
        .iter()
        .map(|fragment| fragment.connections.len())
        .sum::<usize>();
    Ok(format!(
        "Plan {}\nsource {}\nchecked {}\nexpanded {}\ncompletion {:?}\nfragments {}\nplacements {}\nconnections {}\n",
        artifact.plan.plan_id.as_str(),
        artifact.plan.source_document_id.as_str(),
        artifact.plan.checked_plot_id.as_str(),
        artifact.plan.expanded_plot_id.as_str(),
        artifact.plan.completion_policy,
        artifact.plan.fragments.len(),
        placement_count,
        connection_count,
    ))
}

fn inspect_play(bytes: &[u8]) -> Result<String, String> {
    let artifact: conduit_observatory::PlayArtifact =
        serde_json::from_slice(bytes).map_err(|error| format!("decode Play artifact: {error}"))?;
    conduit_observatory::validate_play_artifact(&artifact)?;
    Ok(format!(
        "Play {}\nPlan {}\nHost {}\nBoot {}\nsequence {}\nlifecycle {:?}\nterminal {:?}\nfailure {}\nplacements {}\nconnections {}\n",
        artifact.identity.active_play_id.as_str(),
        artifact.identity.plan_id.as_str(),
        artifact.identity.host_id.as_str(),
        artifact.identity.boot_id.as_str(),
        artifact.identity.play_sequence,
        artifact.play.lifecycle,
        artifact.play.terminal_disposition,
        artifact.play.failure_message.as_deref().unwrap_or("none"),
        artifact.play.placements.len(),
        artifact.play.connections.len(),
    ))
}

fn inspect_sign(bytes: &[u8]) -> Result<String, String> {
    let artifact: conduit_observatory::SignArtifact =
        serde_json::from_slice(bytes).map_err(|error| format!("decode Sign artifact: {error}"))?;
    conduit_observatory::validate_sign_artifact(&artifact)?;
    Ok(format!(
        "Sign {}\nHost {}\nBoot {}\nsequence {}\nPlay {}\nPlan {}\nplacement {}\nconnection {}\npresentation {}\nkind {:?}\n",
        artifact.identity.sign_id.as_str(),
        artifact.identity.host_id.as_str(),
        artifact.identity.boot_id.as_str(),
        artifact.identity.sequence,
        artifact
            .identity
            .active_play_id
            .as_ref()
            .map_or("none", conduit_core::ActivePlayId::as_str),
        artifact
            .sign
            .plan_id
            .as_ref()
            .map_or("none", conduit_core::PlanId::as_str),
        artifact
            .sign
            .placement_id
            .as_ref()
            .map_or("none", conduit_core::PlacementId::as_str),
        artifact
            .sign
            .connection_id
            .as_ref()
            .map_or("none", conduit_core::ConnectionId::as_str),
        artifact
            .sign
            .presentation_id
            .as_ref()
            .map_or("none", conduit_core::PresentationId::as_str),
        artifact.sign.kind,
    ))
}

fn inspect_report(bytes: &[u8]) -> Result<String, String> {
    let snapshot: conduit_observatory::ObservatorySnapshot = serde_json::from_slice(bytes)
        .map_err(|error| format!("decode Observatory report: {error}"))?;
    conduit_observatory::validate_snapshot(&snapshot)?;
    let report = build_report(&snapshot)?;
    Ok(render_text_report(&report))
}

fn inspect_body(bytes: &[u8]) -> Result<String, String> {
    let biography: conduit_body::BodyBiographyEvidence =
        serde_json::from_slice(bytes).map_err(|error| format!("decode Body biography: {error}"))?;
    biography
        .validate()
        .map_err(|error| format!("Body biography refused: {error:?}"))?;
    let present_members = biography
        .membership
        .parts
        .iter()
        .filter(|part| part.current.is_some())
        .count();
    Ok(format!(
        "Body {}\nname {}\nstate {:?}\nmembers {}\npresent members {}\nrecords {}\nwakes {}\n",
        biography.body_id.as_str(),
        biography.friendly_name,
        biography.body.state,
        biography.membership.parts.len(),
        present_members,
        biography.records.len(),
        biography.wakes.len(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_schema_fails_explicitly() {
        let error =
            inspect_json(Path::new("artifact.json"), br#"{"schema":"elsewhere/v1"}"#).unwrap_err();
        assert_eq!(error, "inspection does not yet support schema elsewhere/v1");
    }

    #[test]
    fn malformed_known_schema_is_not_pretty_printed_as_truth() {
        let error = inspect_json(
            Path::new("body.json"),
            br#"{"schema":"conduit.body/biography-evidence@2"}"#,
        )
        .unwrap_err();
        assert!(error.starts_with("decode Body biography:"));
    }
}
