//! Affine initial owners composed before the one final IMAGE ledger.
use super::*;
use crate::kernel_preparation::{ImageMemoryLease, StagedImageMemory};
#[derive(Default)]
pub(crate) struct InitialBases<'a> {
    pub(crate) worker: Option<crate::selected_native_source::image_worker::PreparedImageWorker<'a>>,
    pub(crate) recognition: Option<super::InitialWhisper>,
    pub(crate) memory: Option<StagedImageMemory<'a>>,
    pub(crate) playback: Option<(
        crate::hosted_audio::HostedPlaybackSelection,
        ImageMemoryLease,
    )>,
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub(crate) scanout: Option<Box<crate::selected_scanout::SelectedScanoutBase>>,
    #[cfg(all(target_os = "linux", feature = "linux-display"))]
    pub(crate) native_mask: Option<crate::selected_native_mask::InitialNativeMask>,
}
impl InitialBases<'_> {
    pub(super) fn has_native(&self) -> bool {
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.native_mask.is_some()
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
    pub(super) fn supports_native_implementation(&self, _id: &str) -> bool {
        if self.worker.as_ref().is_some_and(|worker| {
            let entry = &worker.definition().base;
            entry
                .capabilities
                .iter()
                .any(|offer| offer.implementation.implementation_id.as_str() == _id)
        }) {
            return true;
        }

        if self.recognition.as_ref().is_some_and(|recognition| {
            recognition
                .entry
                .capabilities
                .iter()
                .any(|offer| offer.implementation.implementation_id.as_str() == _id)
        }) {
            return true;
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.native_mask.as_ref().is_some_and(|native| {
                native
                    .entry
                    .capabilities
                    .iter()
                    .any(|offer| offer.implementation.implementation_id.as_str() == _id)
            }) || (self.native_mask.is_some() && _id == "conduit.face/tee-kernel@1")
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
    pub(super) fn supports_native_base(&self, _id: &str, _kind: &str, _driver: &str) -> bool {
        if self.worker.as_ref().is_some_and(|worker| {
            let entry = &worker.definition().base;
            entry.base_id.as_str() == _id
                && entry.mechanism_family.as_str() == _kind
                && entry.implementation_id.as_str() == _driver
        }) {
            return true;
        }

        if self.recognition.as_ref().is_some_and(|recognition| {
            recognition.entry.base_id.as_str() == _id
                && recognition.entry.mechanism_family.as_str() == _kind
                && recognition.entry.implementation_id.as_str() == _driver
        }) {
            return true;
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.scanout.as_ref().is_some_and(|scanout| {
                scanout.entry.base_id.as_str() == _id
                    && scanout.entry.mechanism_family.as_str() == _kind
                    && scanout.entry.implementation_id.as_str() == _driver
            })
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
    pub(super) fn owns_native_pool(&self, _id: &str) -> bool {
        if self.worker.as_ref().is_some_and(|worker| {
            let entry = &worker.definition().base;
            entry
                .resources
                .iter()
                .any(|pool| pool.pool_id.as_str() == _id)
        }) {
            return true;
        }

        if self.recognition.as_ref().is_some_and(|recognition| {
            recognition
                .entry
                .resources
                .iter()
                .any(|pool| pool.pool_id.as_str() == _id)
        }) {
            return true;
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        {
            self.native_mask.as_ref().is_some_and(|native| {
                native
                    .entry
                    .resources
                    .iter()
                    .any(|pool| pool.pool_id.as_str() == _id)
            })
        }
        #[cfg(not(all(target_os = "linux", feature = "linux-display")))]
        {
            false
        }
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn project_native(
        &self,
        _config: &StdHostConfig,
        _registry: &mut BaseRegistry,
        _bases: &mut Vec<BaseProviderEntry>,
        _offers: &mut Vec<CapabilityOffer>,
        _classes: &mut BTreeSet<String>,
        _kinds: &mut BTreeSet<String>,
        _drivers: &mut BTreeSet<String>,
    ) -> Result<(), String> {
        if let Some(worker) = &self.worker {
            let entry = &worker.definition().base;
            _registry
                .register(entry.clone())
                .map_err(|error| format!("selected worker registry: {error:?}"))?;
            _bases.push(entry.clone());
            _classes.extend(
                entry
                    .resources
                    .iter()
                    .map(|pool| pool.class_id.as_str().to_owned()),
            );
            _kinds.insert(entry.mechanism_family.as_str().to_owned());
            _drivers.insert(entry.implementation_id.as_str().to_owned());
        }
        if let Some(recognition) = &self.recognition {
            _registry
                .register(recognition.entry.clone())
                .map_err(|error| format!("selected recognizer registry:{error:?}"))?;
            _bases.push(recognition.entry.clone());
            _classes.extend(
                recognition
                    .entry
                    .resources
                    .iter()
                    .map(|pool| pool.class_id.as_str().to_owned()),
            );
            _kinds.insert("speech/recognizer".into());
            _drivers.insert(conduit_std_offers::WHISPER_CLIP_SPEECH_IMPLEMENTATION.into());
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(native) = &self.native_mask {
            let offered = native.offers(_config);
            _offers.extend(offered.capabilities.into_iter().filter(|offer| {
                !native
                    .entry
                    .capabilities
                    .iter()
                    .any(|owned| owned.capability_id == offer.capability_id)
            }));
            _registry
                .register(native.entry.clone())
                .map_err(|error| format!("selected native registry:{error:?}"))?;
            _bases.push(native.entry.clone());
            _classes.extend(
                native
                    .entry
                    .resources
                    .iter()
                    .map(|pool| pool.class_id.as_str().to_owned()),
            );
            _kinds.insert("display/scanout".into());
            _drivers.insert("display/linear-framebuffer@1".into());
        }
        Ok(())
    }
    pub(super) fn finish_native(&mut self, _host: &mut StdHost) -> Result<(), String> {
        if let Some(worker) = self.worker.take() {
            worker.finish(_host)?;
        }
        if let Some(recognition) = self.recognition.take() {
            _host.speech_recognition = Some(recognition.adapter);
        }
        #[cfg(all(target_os = "linux", feature = "linux-display"))]
        if let Some(mut scanout) = self.scanout.take() {
            let initial = self
                .native_mask
                .take()
                .ok_or("selected scanout lacks fixed native owner")?;
            scanout.native_mask = Some(
                crate::selected_native_mask::SelectedNativeMaskBase::initialize(_host, initial)?,
            );
            _host.selected_scanout = Some(scanout);
        }
        Ok(())
    }
}
