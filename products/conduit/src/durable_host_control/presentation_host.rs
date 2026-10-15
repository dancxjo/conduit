//! Foreground access only to an explicitly prepared presentation-only Host.
use super::{DurableHostRuntime, HostSource};
impl DurableHostRuntime {
    pub(crate) fn attach_presentation_terminal(
        &mut self,
        stream: std::os::unix::net::UnixStream,
    ) -> Result<(), String> {
        self.presentation_host_mut()?.attach_terminal_mask(stream)?;
        if let Some(equipment) = &mut self.selected_speech_equipment {
            let HostSource::Bare(host) = &self.host else {
                return Err("selected presentation speech has no foreground Host".into());
            };
            equipment.advance_offer_generation(host)
        } else {
            Ok(())
        }
    }

    pub(crate) fn presentation_host_mut(
        &mut self,
    ) -> Result<&mut conduit_std_host::StdHost, String> {
        match &mut self.host {
            HostSource::Bare(host) => Ok(host),
            _ => Err("presentation requires a Host without a local Body".into()),
        }
    }
}
