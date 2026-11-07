//! Authenticated owner preparation; public input never supplies executable tables.
use super::*;
impl DurableHostRuntime {
    pub(in crate::durable_host_control) fn owned_body_prepare_usb(
        &mut self,
        port: &Path,
        member: &MemberPresentationBasis,
        source: String,
        entries: &[String],
        native_capabilities: &[String],
    ) -> Result<serde_json::Value, String> {
        let HostSource::Body {
            owner,
            running: None,
            ..
        } = &mut self.host
        else {
            return Err("remote preparation requires the idle live installed Body owner".into());
        };
        owner.submit_remote_workset_usb(port, member, source, entries, native_capabilities)
    }
}
pub(crate) fn prepare_usb(
    state: &Path,
    port: PathBuf,
    member: MemberPresentationBasis,
    source: String,
    entries: Vec<String>,
    native_capabilities: Vec<String>,
) -> Result<serde_json::Value, String> {
    #[cfg(unix)]
    {
        match call(
            state,
            Request::BodyPrepareUsb {
                protocol: PROTOCOL,
                token: token(state)?,
                port,
                member: Box::new(member),
                source,
                entries,
                native_capabilities,
            },
        )? {
            Response::BodyWorksetSubmitted {
                protocol: PROTOCOL,
                report,
            } => Ok(report),
            Response::Refused { code, .. } => Err(code),
            _ => Err("wrong remote workset response".into()),
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (state, port, member, source, entries, native_capabilities);
        Err("remote workset preparation requires local Unix owner control".into())
    }
}
