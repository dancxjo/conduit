//! Custody of the installed Host across one ordinary Body Play.
use conduit_core::HostAdvertisement;
use conduit_std_host::StdHost;
use std::{
    ops::{Deref, DerefMut},
    path::Path,
};

/// The owner keeps its Boot advertisement while its one Host executes the
/// admitted Body Play on a worker. No second Host is constructed.
pub(crate) struct OwnerHost {
    current: Option<StdHost>,
    advertised: HostAdvertisement,
}

impl OwnerHost {
    pub(super) fn new(host: StdHost) -> Self {
        Self {
            advertised: host.advertisement().clone(),
            current: Some(host),
        }
    }

    pub(crate) fn advertisement(&self) -> &HostAdvertisement {
        self.current
            .as_ref()
            .map(StdHost::advertisement)
            .unwrap_or(&self.advertised)
    }

    pub(crate) fn take_for_play(&mut self) -> Result<StdHost, String> {
        let host = self
            .current
            .take()
            .ok_or("Body Play already owns the Host")?;
        self.advertised = host.advertisement().clone();
        Ok(host)
    }

    pub(crate) fn restore_after_play(&mut self, host: StdHost) -> Result<(), String> {
        if self.current.is_some() || host.advertisement() != &self.advertised {
            return Err(
                "Body Play returned a different Host advertisement or duplicate Host".into(),
            );
        }
        self.advertised = host.advertisement().clone();
        self.current = Some(host);
        Ok(())
    }

    pub(crate) fn transition_todo_checkpoint_offer(
        &mut self,
        root: &Path,
        content: conduit_core::ResourceContentRequirement,
    ) -> Result<(), String> {
        let host = self
            .current
            .as_mut()
            .ok_or("Todo checkpoint transition requires an idle Host")?;
        host.transition_todo_checkpoint_offer(root, content)?;
        self.advertised = host.advertisement().clone();
        Ok(())
    }

    pub(crate) fn current(&self) -> &StdHost {
        self.current
            .as_ref()
            .expect("Host effects require an idle owner")
    }

    pub(crate) fn current_mut(&mut self) -> &mut StdHost {
        self.current
            .as_mut()
            .expect("Host effects require an idle owner")
    }

    pub(crate) fn is_playing(&self) -> bool {
        self.current.is_none()
    }
}

impl Deref for OwnerHost {
    type Target = StdHost;

    fn deref(&self) -> &Self::Target {
        self.current()
    }
}

impl DerefMut for OwnerHost {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.current_mut()
    }
}
