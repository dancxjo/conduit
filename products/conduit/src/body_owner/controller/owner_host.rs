//! Retain original IMAGE preparation custody while the sole Host is loaned.
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
    root_preparation: Option<conduit_std_host::body_execution::ImageRootPreparationOwner>,
}

impl OwnerHost {
    pub(crate) fn prepare(
        host: &StdHost,
    ) -> Result<Option<conduit_std_host::body_execution::ImageRootPreparationOwner>, String> {
        if host.image_identity().is_some() {
            Ok(Some(host.image_root_preparation_owner()?))
        } else {
            Ok(None)
        }
    }

    pub(super) fn new(host: StdHost) -> Result<Self, String> {
        let preparation = Self::prepare(&host)?;
        Self::from_prepared(host, preparation).map_err(|(_, error)| error)
    }

    // On verification failure return the original Host, so a publication
    // caller can restore custody even after its recoverable journal exists.
    #[expect(
        clippy::result_large_err,
        reason = "Return original affine Host on refusal without allocating on the failure path"
    )]
    pub(crate) fn from_prepared(
        host: StdHost,
        root_preparation: Option<conduit_std_host::body_execution::ImageRootPreparationOwner>,
    ) -> Result<Self, (StdHost, String)> {
        if let Err(error) = Self::verify_preparation(&host, &root_preparation) {
            return Err((host, error));
        }
        Ok(Self {
            advertised: host.advertisement().clone(),
            current: Some(host),
            root_preparation,
        })
    }

    /// Birth already owns this Box. Refusal returns that exact allocation and
    /// its affine Host, including after a recoverable journal was retained.
    pub(crate) fn from_prepared_boxed(
        host: Box<StdHost>,
        root_preparation: Option<conduit_std_host::body_execution::ImageRootPreparationOwner>,
    ) -> Result<Self, (Box<StdHost>, String)> {
        if let Err(error) = Self::verify_preparation(&host, &root_preparation) {
            return Err((host, error));
        }
        Ok(Self {
            advertised: host.advertisement().clone(),
            current: Some(*host),
            root_preparation,
        })
    }

    fn verify_preparation(
        host: &StdHost,
        root_preparation: &Option<conduit_std_host::body_execution::ImageRootPreparationOwner>,
    ) -> Result<(), String> {
        match (root_preparation, host.image_identity()) {
            (Some(owner), Some(_)) => owner.verify_host(host),
            (None, None) => Ok(()),
            _ => Err("Birth changed original IMAGE preparation lifetime".into()),
        }
    }

    pub(crate) fn has_image_custody(&self) -> bool {
        self.root_preparation.is_some()
    }
    pub(crate) fn optional_root_preparation(
        &self,
    ) -> Option<&conduit_std_host::body_execution::ImageRootPreparationOwner> {
        self.root_preparation.as_ref()
    }

    pub(crate) fn root_preparation(
        &self,
    ) -> Result<&conduit_std_host::body_execution::ImageRootPreparationOwner, String> {
        self.root_preparation
            .as_ref()
            .ok_or_else(|| "Root preparation requires original IMAGE custody".into())
    }

    fn verify_root_preparation_host(&self, host: &StdHost) -> Result<(), String> {
        if self.has_image_custody() {
            self.root_preparation()?.verify_host(host)
        } else if host.image_identity().is_none() {
            Ok(())
        } else {
            Err("Body Play changed original IMAGE preparation lifetime".into())
        }
    }

    pub(crate) fn advertisement(&self) -> &HostAdvertisement {
        self.current
            .as_ref()
            .map(StdHost::advertisement)
            .unwrap_or(&self.advertised)
    }

    pub(crate) fn take_for_play(&mut self) -> Result<StdHost, String> {
        self.verify_root_preparation_host(
            self.current
                .as_ref()
                .ok_or("Body Play already owns the Host")?,
        )?;
        let host = self
            .current
            .take()
            .ok_or("Body Play already owns the Host")?;
        self.advertised = host.advertisement().clone();
        Ok(host)
    }

    pub(crate) fn restore_after_play(&mut self, host: StdHost) -> Result<(), String> {
        if self.current.is_some() || host.advertisement() != &self.advertised {
            if let Some(owner) = &self.root_preparation {
                owner.invalidate();
            }
            return Err(
                "Body Play returned a different Host advertisement or duplicate Host".into(),
            );
        }
        if let Err(error) = self.verify_root_preparation_host(&host) {
            if let Some(owner) = &self.root_preparation {
                owner.invalidate();
            }
            return Err(error);
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

impl Drop for OwnerHost {
    fn drop(&mut self) {
        if let Some(owner) = self.optional_root_preparation() {
            owner.invalidate();
        }
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

#[cfg(test)]
#[path = "owner_host/tests.rs"]
mod tests;
