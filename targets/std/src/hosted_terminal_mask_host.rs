//! A foreground terminal connection is a selected, short-lived I/O provider
//! of this exact std Host. The background service has no ambient terminal.
use crate::{kernel_preparation::KernelResourceLedger, terminal_mask_execution, StdHost};
use conduit_core::OfferGeneration;
use std::{
    io,
    os::{fd::AsRawFd, unix::net::UnixStream},
    time::Duration,
};

#[path = "hosted_terminal_mask_host/effect.rs"]
mod effect;
pub use effect::{receive_terminal_frame_and_ack, TerminalFrameReceipt};

impl StdHost {
    /// Prepare an ordinary terminal Mask only while this Host still owns the
    /// selected provider. The returned executor is fenced by the current
    /// advertisement; the owner must recheck before every render/ack.
    pub fn prepare_terminal_mask_execution(
        &self,
    ) -> Result<terminal_mask_execution::HostedTerminalMaskExecution, String> {
        if !self.terminal_attachment_is_live()? {
            return Err("terminal attachment is unavailable".into());
        }
        terminal_mask_execution::HostedTerminalMaskExecution::new_attached(&self.advertisement)
            .map_err(|error| format!("plan attached terminal Mask: {error:?}"))
    }

    /// The caller must authenticate and retain a foreground terminal provider
    /// before handing over its live connection. No terminal offer survives
    /// detachment, and each transition invalidates plans from the prior offer
    /// generation. This method does not acknowledge a Show.
    pub fn attach_terminal_mask(&mut self, stream: UnixStream) -> Result<(), String> {
        if self.terminal_attachment.is_some() || !self.kernel_resources.is_idle() {
            return Err("terminal attachment requires an idle, unattached Host".into());
        }
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("bound terminal attachment read: {error}"))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| format!("bound terminal attachment write: {error}"))?;
        if !peer_is_live(&stream)? {
            return Err("terminal attachment peer already closed".into());
        }
        let mut advertisement = self.advertisement.clone();
        let capabilities = terminal_mask_execution::planning::attached_terminal_capabilities();
        let resources = terminal_mask_execution::planning::attached_terminal_resources();
        if capabilities.iter().any(|offer| {
            advertisement
                .capabilities
                .iter()
                .any(|existing| existing.capability_id == offer.capability_id)
        }) || resources.iter().any(|offer| {
            advertisement
                .resources
                .iter()
                .any(|existing| existing.pool_id == offer.pool_id)
        }) {
            return Err("terminal attachment offer conflicts with current Host".into());
        }
        advertisement.capabilities.extend(capabilities);
        advertisement.resources.extend(resources);
        advertisement.resources.sort();
        crate::normalize_capability_offers(&mut advertisement.capabilities)?;
        advertisement.offer_generation = next_generation(advertisement.offer_generation)?;
        let ledger = KernelResourceLedger::new(&advertisement)?;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        self.terminal_attachment = Some(stream);
        Ok(())
    }

    /// On EOF or any provider failure, the owner must call this before making
    /// another terminal availability claim. Already sealed plans become stale.
    pub fn detach_terminal_mask(&mut self) -> Result<(), String> {
        if self.terminal_attachment.is_none() || !self.kernel_resources.is_idle() {
            return Err("terminal detachment requires an idle attached Host".into());
        }
        let mut advertisement = self.advertisement.clone();
        let capability_ids = terminal_mask_execution::planning::attached_terminal_capabilities()
            .into_iter()
            .map(|offer| offer.capability_id)
            .collect::<Vec<_>>();
        let pool_ids = terminal_mask_execution::planning::attached_terminal_resources()
            .into_iter()
            .map(|offer| offer.pool_id)
            .collect::<Vec<_>>();
        advertisement
            .capabilities
            .retain(|offer| !capability_ids.contains(&offer.capability_id));
        advertisement
            .resources
            .retain(|offer| !pool_ids.contains(&offer.pool_id));
        advertisement.offer_generation = next_generation(advertisement.offer_generation)?;
        let ledger = KernelResourceLedger::new(&advertisement)?;
        self.terminal_attachment = None;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        Ok(())
    }

    pub fn terminal_attachment_mut(&mut self) -> Option<&mut UnixStream> {
        self.terminal_attachment.as_mut()
    }

    /// Run the ordinary planned Mask through this Host's attached terminal.
    /// Only an actual foreground write+flush acknowledgement can make its
    /// Show Available. Retain the execution until its typed input Fore is
    /// satisfied or the route is retired.
    pub fn present_attached_terminal_face_with_interaction(
        &mut self,
        face: &conduit_presentation::Presentation,
    ) -> Result<
        (
            conduit_presentation::MaskShow,
            terminal_mask_execution::HostedTerminalMaskExecution,
        ),
        String,
    > {
        use crate::terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution};
        let mut execution = match self.prepare_terminal_mask_execution() {
            Ok(execution) => execution,
            Err(error) => {
                if self.terminal_attachment.is_some() {
                    self.detach_terminal_mask()?;
                }
                return Err(error);
            }
        };
        let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24)
            .map_err(|error| format!("prepare attached terminal Face: {error:?}"))?;
        let result = (|| {
            let prepared = execution
                .begin_render(face)
                .map_err(|error| format!("begin attached terminal Mask: {error:?}"))?;
            let stream = self
                .terminal_attachment
                .as_mut()
                .ok_or("terminal attachment vanished before effect")?;
            let mut writer = effect::AttachedTerminalWriter::new(stream, &prepared);
            let receipt = mask
                .render(&mut writer, &prepared)
                .map_err(|error| format!("render attached terminal Face: {error:?}"))?;
            if !self.terminal_attachment_is_live()? {
                return Err("terminal attachment closed before Show acknowledgement".into());
            }
            let available = execution
                .complete_render(&receipt)
                .map_err(|error| format!("complete attached terminal Mask: {error:?}"))?;
            mask.bind_show(receipt, available.clone())
                .map_err(|error| format!("bind attached terminal Show: {error:?}"))?;
            Ok(available)
        })();
        match result {
            Ok(show) => Ok((show, execution)),
            Err(error) => {
                let _ = execution.cancel();
                self.detach_terminal_mask()?;
                Err(error)
            }
        }
    }

    /// Present once and close the interaction Fore for callers that only
    /// requested a readable Show.
    pub fn present_attached_terminal_face(
        &mut self,
        face: &conduit_presentation::Presentation,
    ) -> Result<conduit_presentation::MaskShow, String> {
        use crate::terminal_face_mask::TerminalMaskExecution;
        let (show, mut execution) = self.present_attached_terminal_face_with_interaction(face)?;
        if let Err(error) = execution.close_without_input() {
            self.detach_terminal_mask()?;
            return Err(format!("close attached terminal Mask input: {error:?}"));
        }
        Ok(show)
    }

    /// A non-consuming liveness check for the owner event loop. A false result
    /// must immediately retire the selected provider and its prior Shows.
    pub fn terminal_attachment_is_live(&self) -> Result<bool, String> {
        self.terminal_attachment
            .as_ref()
            .map(peer_is_live)
            .transpose()
            .map(|live| live.unwrap_or(false))
    }
}

fn peer_is_live(stream: &UnixStream) -> Result<bool, String> {
    let mut byte = [0];
    // SAFETY: `byte` is a valid writable one-byte buffer and the fd remains
    // owned by `stream` throughout this non-consuming, nonblocking syscall.
    let received = unsafe {
        libc::recv(
            stream.as_raw_fd(),
            byte.as_mut_ptr().cast(),
            byte.len(),
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    match received {
        0 => Ok(false),
        amount if amount > 0 => Ok(true),
        _ => match io::Error::last_os_error() {
            error if error.kind() == io::ErrorKind::WouldBlock => Ok(true),
            error if error.kind() == io::ErrorKind::ConnectionReset => Ok(false),
            error => Err(format!("check terminal attachment peer: {error}")),
        },
    }
}

fn next_generation(current: OfferGeneration) -> Result<OfferGeneration, String> {
    current
        .0
        .checked_add(1)
        .map(OfferGeneration)
        .ok_or_else(|| "terminal Host offer generation exhausted".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal_mask_execution::HostedTerminalMaskExecution;

    #[test]
    fn only_a_live_attachment_can_plan_on_the_actual_host_offer() {
        let mut host = StdHost::new();
        let initial = host.advertisement().clone();
        assert!(HostedTerminalMaskExecution::new_attached(&initial).is_err());
        let (service, _foreground) = UnixStream::pair().unwrap();
        host.attach_terminal_mask(service).unwrap();
        assert!(host.terminal_attachment_is_live().unwrap());
        let attached = host.advertisement().clone();
        assert_eq!(attached.host_id, initial.host_id);
        assert_eq!(attached.boot_id, initial.boot_id);
        assert!(attached.offer_generation.0 > initial.offer_generation.0);
        let mut execution = host.prepare_terminal_mask_execution().unwrap();
        assert_eq!(
            execution.planned_mask().plan.fragments[0].host_id,
            initial.host_id
        );
        assert_eq!(
            execution.planned_mask().plan.fragments[0].boot_id,
            initial.boot_id
        );
        assert_eq!(
            execution.planned_mask().plan.fragments[0].offer_generation,
            attached.offer_generation
        );
        assert!(host
            .attach_terminal_mask(UnixStream::pair().unwrap().0)
            .is_err());
        host.detach_terminal_mask().unwrap();
        let retired = host.advertisement();
        assert_eq!(retired.host_id, initial.host_id);
        assert_eq!(retired.boot_id, initial.boot_id);
        assert!(retired.offer_generation.0 > attached.offer_generation.0);
        assert!(execution.validate_current_host(retired).is_err());
        assert!(HostedTerminalMaskExecution::new_attached(retired).is_err());
    }

    #[test]
    fn closed_peer_is_never_advertised() {
        let mut host = StdHost::new();
        let initial = host.advertisement().clone();
        let (service, foreground) = UnixStream::pair().unwrap();
        drop(foreground);
        assert!(host.attach_terminal_mask(service).is_err());
        assert_eq!(host.advertisement(), &initial);
    }

    #[test]
    fn lost_attached_peer_retires_its_offers_and_plan_basis() {
        let mut host = StdHost::new();
        let (service, foreground) = UnixStream::pair().unwrap();
        host.attach_terminal_mask(service).unwrap();
        let attached = host.advertisement().clone();
        drop(foreground);
        assert!(!host.terminal_attachment_is_live().unwrap());
        assert!(host.prepare_terminal_mask_execution().is_err());
        host.detach_terminal_mask().unwrap();
        assert!(HostedTerminalMaskExecution::new_attached(host.advertisement()).is_err());
        assert_ne!(
            host.advertisement().offer_generation,
            attached.offer_generation
        );
    }
}
