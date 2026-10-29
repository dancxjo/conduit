//! Private transactional Root installation; callbacks provide no grant.
use super::*;
use crate::kernel_preparation::KernelResourceLedger;
impl crate::StdHost {
    pub(super) fn stage_native_source(
        &self,
        definition: &NativeSourceDefinition,
        worker_bytes: Option<u32>,
    ) -> Result<(BaseRegistry, HostAdvertisement, KernelResourceLedger), String> {
        if !self.kernel_resources.is_idle() || self.native_source.is_some() {
            return Err("native Source installation requires an idle empty slot".into());
        }
        validate_definition(definition, &self.advertisement, worker_bytes)?;
        let mut registry = self.base_registry.clone();
        if let Some(previous) = registry
            .entries()
            .iter()
            .find(|entry| entry.base_id == definition.base.base_id)
            .cloned()
        {
            if previous.lifecycle != BaseLifecycle::Stopped {
                return Err("native Base is not actually retired".into());
            }
            registry.replace(
                &previous.base_id,
                &previous.provider_instance_id,
                previous.provider_generation,
                definition.base.clone(),
            )
        } else {
            registry.register(definition.base.clone())
        }
        .map_err(|e| format!("native Source registration: {e:?}"))?;
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation.0 = advertisement
            .offer_generation
            .0
            .checked_add(1)
            .ok_or("native Source offer generation exhausted")?;
        registry
            .project_base_ready_into(&definition.base.base_id, &mut advertisement)
            .map_err(|e| format!("native Source advertisement: {e:?}"))?;
        let ledger = KernelResourceLedger::new(&advertisement)?;
        Ok((registry, advertisement, ledger))
    }
    /// Initialize a retained fixed-slot worker only after actual Base backing
    /// admission. Ready offers become observable only after native readiness.
    ///
    /// # Safety
    /// The independent definition, current authorized grant, memory offer and
    /// native provider must correspond to the actually initialized Host and
    /// checked configuration. The provider must meet its cooperative deadline,
    /// cancellation and finite allocation bounds; received Plans grant nothing.
    #[allow(clippy::result_large_err)]
    pub unsafe fn install_native_source_worker(
        &mut self,
        definition: NativeSourceDefinition,
        budget: NativeWorkerBudget,
        provider: Box<dyn NativeSourceWorkerProvider>,
    ) -> Result<
        (),
        (
            NativeSourceDefinition,
            Box<dyn NativeSourceWorkerProvider>,
            String,
        ),
    > {
        if definition.base.enforcement_class != BaseEnforcementClass::Cooperative
            || definition.base.capabilities.first().is_none_or(|offer| {
                offer.implementation.execution_profile_id.as_str() != NATIVE_SOURCE_WORKER_PROFILE
            })
        {
            return Err((
                definition,
                provider,
                "native Source worker requires its explicit native execution profile".into(),
            ));
        }
        let bytes = match budget.backing_bytes(definition.input_bytes, definition.output_bytes) {
            Ok(v) => v,
            Err(e) => {
                return Err((
                    definition,
                    provider,
                    format!("native worker profile: {e:?}"),
                ))
            }
        };
        let (registry, advertisement, mut ledger) =
            match self.stage_native_source(&definition, Some(bytes)) {
                Ok(v) => v,
                Err(e) => return Err((definition, provider, e)),
            };
        let backing = match ledger.reserve_base_backing(
            &advertisement,
            &registry,
            &definition.base.provider_instance_id,
            definition.base.provider_generation,
            &definition.base.resources[1].pool_id,
            bytes,
        ) {
            Ok(v) => v,
            Err(e) => return Err((definition, provider, e)),
        };
        // No worker Arc, fixed buffers or OS thread exists before this point.
        let worker = match worker::NativeWorker::spawn(
            provider,
            budget,
            definition.input_bytes,
            definition.output_bytes,
        ) {
            Ok(v) => v,
            Err((provider, e)) => {
                // spawn returns only after any started thread has joined.
                let reason = format!("native worker startup: {e:?}");
                let _ = ledger.release_base_backing(backing);
                return Err((definition, provider, reason));
            }
        };
        self.base_registry = registry;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        self.native_source = Some(InitializedNativeSource {
            definition,
            provider: NativeProvider::Worker(worker),
            backing: Some(backing),
        });
        Ok(())
    }
}
impl crate::StdHost {
    pub fn native_source_worker_inspection(&self) -> Option<NativeWorkerInspection> {
        let source = self.native_source.as_ref()?;
        let NativeProvider::Worker(worker) = &source.provider else {
            return None;
        };
        let backing = source.backing.as_ref()?;
        Some(NativeWorkerInspection {
            backing: worker.observation(),
            reserved_bytes: backing.bytes(),
            idle_backing_is_reserved: self.kernel_resources.only_base_backing_is_reserved(backing),
            cancellation_overdue: worker.cancellation_overdue(),
        })
    }
    /// Request shutdown; Busy retains the actual thread, provider and memory
    /// reservation. Only a joined exit can return the owner and free backing.
    pub fn retire_native_source_worker(&mut self) -> Result<RetiredNativeSource, String> {
        let source = self
            .native_source
            .as_mut()
            .ok_or("no initialized native Source")?;
        let backing = source
            .backing
            .as_ref()
            .ok_or("the native Source has no worker backing")?;
        if !self.kernel_resources.only_base_backing_is_reserved(backing) {
            return Err("native worker retirement requires all child Plays quiescent".into());
        }
        let next_generation = self
            .advertisement
            .offer_generation
            .0
            .checked_add(1)
            .ok_or("native retirement generation exhausted")?;
        let mut advertisement = self.advertisement.clone();
        advertisement.capabilities.retain(|offer| {
            !source
                .definition
                .base
                .capabilities
                .iter()
                .any(|own| own.capability_id == offer.capability_id)
        });
        advertisement.resources.retain(|offer| {
            !source
                .definition
                .base
                .resources
                .iter()
                .any(|own| own.pool_id == offer.pool_id)
        });
        advertisement.bases.retain(|base| {
            base.provider_instance_id != source.definition.base.provider_instance_id
        });
        advertisement.offer_generation = OfferGeneration(next_generation);
        let ledger = KernelResourceLedger::new(&advertisement)?;
        let mut registry = self.base_registry.clone();
        registry
            .set_lifecycle(
                &source.definition.base.base_id,
                &source.definition.base.provider_instance_id,
                source.definition.base.provider_generation,
                BaseLifecycle::Stopped,
            )
            .map_err(|e| format!("native Base retirement registry: {e:?}"))?;
        let NativeProvider::Worker(worker) = &mut source.provider else {
            return Err("native Source is not worker-owned".into());
        };
        let provider = worker
            .retire()
            .map_err(|e| format!("native worker retirement retains its owner: {e:?}"))?;
        // Mutable Host ownership and the check above prevent ledger changes
        // between actual joined retirement and this sole affine release.
        let source = self.native_source.take().expect("retained source above");
        self.kernel_resources
            .release_base_backing(source.backing.expect("worker backing above"))
            .unwrap_or_else(|_| panic!("exact native backing changed during exclusive retirement"));
        self.base_registry = registry;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        Ok(RetiredNativeSource {
            definition: source.definition,
            provider,
        })
    }
}
