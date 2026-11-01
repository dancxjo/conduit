//! Authenticated installed-owner Source replacement; no new Body or autoStart.
use super::*;
impl DurableHostRuntime {
    pub(in super::super) fn replace_owned_plot(
        &mut self,
        source: &str,
        revision: u64,
        checked: &str,
    ) -> Result<serde_json::Value, String> {
        match &mut self.host {
            HostSource::Body {
                owner,
                root,
                running: None,
            } => owner.replace_plot(root, source, revision, checked),
            _ => Err("Plot replacement requires an idle installed Body owner".into()),
        }
    }
}
#[cfg(unix)]
pub(crate) fn replace_plot(
    state_dir: &Path,
    source: &Path,
    revision: u64,
    checked: &str,
) -> Result<(), String> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(source)
        .map_err(|e| e.to_string())?
        .take(256 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 256 * 1024 {
        return Err("replacement Source exceeds its finite bound".into());
    }
    let source = String::from_utf8(bytes).map_err(|_| "replacement Source is not UTF-8")?;
    match call(
        state_dir,
        Request::BodyReplacePlot {
            protocol: PROTOCOL,
            token: super::token(state_dir)?,
            source,
            expected_workload_revision: revision,
            expected_checked_plot_id: checked.into(),
        },
    )? {
        Response::BodyTruth {
            protocol: PROTOCOL,
            truth,
        } => {
            println!(
                "{}",
                serde_json::to_string(&truth).map_err(|e| e.to_string())?
            );
            Ok(())
        }
        Response::Refused { code, .. } if code.contains("outcome-unknown") => Err(code),
        Response::Refused { code, .. } => Err(format!("Body Plot replacement refused: {code}")),
        _ => Err("Body owner returned the wrong Plot replacement response".into()),
    }
}
#[cfg(not(unix))]
pub(crate) fn replace_plot(
    _state_dir: &Path,
    _source: &Path,
    _revision: u64,
    _checked: &str,
) -> Result<(), String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}
