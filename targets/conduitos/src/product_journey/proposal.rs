//! Native offers prepare partitions; the canonical session publishes Wake/Plan.
use super::*;

impl ProductJourney {
    pub(super) fn wake(
        &mut self,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
    ) -> Result<(), JourneyError> {
        if self.body().ok_or(JourneyError::BodyAbsent)?.state != BodyState::Lulled {
            return Err(JourneyError::InvalidTransition);
        }
        self.propose(identities, offer, build_id, None)
    }

    pub(super) fn propose(
        &mut self,
        identities: &BootIdentities,
        offer: &HostOffer<'_>,
        build_id: &str,
        control: Option<crate::mask_control::MaskControl>,
    ) -> Result<(), JourneyError> {
        if crate::identity::hex(&identities.host) != self.host_id.as_str()
            || crate::identity::hex(&identities.boot) != self.boot_id.as_str()
            || offer.generation != self.offer_generation.0
        {
            return Err(JourneyError::WrongTarget);
        }
        let mut session = self.session.clone().ok_or(JourneyError::BodyAbsent)?;
        let partitions = native_workset::propose_partitions(
            &session.evidence().body.workset,
            identities,
            offer,
            build_id,
        )
        .map_err(JourneyError::Workset)?;
        let control = if self.plots.contains(&Some(NativePlot::Patchbay)) {
            Some(match control {
                Some(control) => control,
                None => match self.mask_control.clone() {
                    Some(control) => control,
                    None => crate::mask_control::MaskControl::graphical(
                        self.host_id.clone(),
                        self.boot_id.clone(),
                        self.surface_provider.clone(),
                    )
                    .map_err(|_| JourneyError::Kernel)?,
                },
            })
        } else {
            None
        };
        let masks = if let Some(control) = &control {
            let selector = crate::mask_control::patchbay_partition_selector(&partitions)
                .map_err(|_| JourneyError::Kernel)?;
            alloc::vec![
                control
                    .topology(selector)
                    .map_err(|_| JourneyError::Kernel)?
            ]
        } else {
            Vec::new()
        };
        session
            .propose_with_masks(partitions, masks, &self.host_id, &self.boot_id)
            .map_err(JourneyError::Lifecycle)?;
        Self::require_archive_storage(&session)?;
        self.session = Some(session);
        self.mask_control = control;
        self.status = JourneyStatus::Awake;
        Ok(())
    }
}
