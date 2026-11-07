//! Pay for preparation before running native initialization; no IMAGE bypass.
use super::*;
#[derive(Clone, Copy, Debug)]
pub struct ImmediateSourcePreparationBudget {
    /// Independently configured conservative temporary backing ceiling.
    pub preparation_bytes: u32,
    /// Backing retained until the actual immediate provider is retired.
    pub retained_bytes: u32,
}
impl crate::StdHost {
    /// Stage registration and reserve the independently configured memory pool
    /// before invoking the factory. No offer becomes visible until success.
    /// Accounting is not allocator/RSS containment; compiler child limits remain
    /// separately enforced. Checked IMAGE reconfiguration remains forbidden.
    ///
    /// # Safety
    /// The definition/grant must describe actual independently authorized native
    /// construction. The factory's complete managed backing must fit its declared
    /// ceiling and return only when all temporary work has quiesced. The returned
    /// immediate provider and its finite non-unwinding retirement must fit the
    /// retained charge and the existing ImmediateNativeSource contract.
    #[allow(clippy::result_large_err)]
    pub unsafe fn prepare_and_install_immediate_native_source(
        &mut self,
        definition: NativeSourceDefinition,
        budget: ImmediateSourcePreparationBudget,
        prepare: impl FnOnce() -> Result<Box<dyn ImmediateNativeSource>, String>,
    ) -> Result<(), (NativeSourceDefinition, String)> {
        if budget.retained_bytes == 0 || budget.retained_bytes > budget.preparation_bytes {
            return Err((
                definition,
                "native Source preparation/retention budget refused".into(),
            ));
        }
        let (registry, advertisement, mut ledger) =
            match self.stage_native_source(&definition, Some(budget.preparation_bytes)) {
                Ok(staged) => staged,
                Err(reason) => return Err((definition, reason)),
            };
        let mut backing = match ledger.reserve_base_backing(
            &advertisement,
            &registry,
            &definition.base.provider_instance_id,
            definition.base.provider_generation,
            &definition.base.resources[1].pool_id,
            budget.preparation_bytes,
        ) {
            Ok(backing) => backing,
            Err(reason) => return Err((definition, reason)),
        };
        let provider = match prepare() {
            Ok(provider) => provider,
            Err(reason) => {
                // The factory has returned with actual temporary work quiescent.
                let _ = ledger.release_base_backing(backing);
                return Err((definition, reason));
            }
        };
        if let Err(reason) =
            ledger.retain_base_preparation_bytes(&mut backing, budget.retained_bytes as usize)
        {
            drop(provider);
            let _ = ledger.release_base_backing(backing);
            return Err((definition, reason));
        }
        self.base_registry = registry;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        self.native_source = Some(InitializedNativeSource {
            definition,
            provider: NativeProvider::Immediate(provider),
            backing: Some(backing),
            _image_memory: None,
            _metadata_memory: None,
        });
        Ok(())
    }

    /// Retire an immediate owner only while its child Plays are quiescent.
    /// The actual provider is dropped before its backing charge is released.
    ///
    /// # Safety
    /// The independently initialized provider must satisfy the finite retirement
    /// contract given at initialization; dropping it cannot start background work
    /// or unwind. No received descriptor or observation supplies that proof.
    pub unsafe fn retire_immediate_native_source(&mut self) -> Result<(), String> {
        self.kernel_resources.require_reference_reconfiguration()?;
        let source = self
            .native_source
            .as_ref()
            .ok_or("no initialized native Source")?;
        if !matches!(source.provider, NativeProvider::Immediate(_)) {
            return Err("native Source requires its actual worker retirement path".into());
        }
        let quiescent = match &source.backing {
            Some(backing) => self.kernel_resources.only_base_backing_is_reserved(backing),
            None => self.kernel_resources.is_idle(),
        };
        if !quiescent {
            return Err("native Source retirement requires quiescent child Plays".into());
        }
        let mut advertisement = self.advertisement.clone();
        advertisement.offer_generation.0 = advertisement
            .offer_generation
            .0
            .checked_add(1)
            .ok_or("native Source generation exhausted")?;
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
        let ledger = self.kernel_resources.replacement_for(&advertisement)?;
        let mut registry = self.base_registry.clone();
        registry
            .set_lifecycle(
                &source.definition.base.base_id,
                &source.definition.base.provider_instance_id,
                source.definition.base.provider_generation,
                BaseLifecycle::Stopped,
            )
            .map_err(|error| format!("native Source retirement: {error:?}"))?;
        let source = self.native_source.take().expect("source checked above");
        drop(source.provider);
        if let Some(backing) = source.backing {
            self.kernel_resources
                .release_base_backing(backing)
                .unwrap_or_else(|_| {
                    panic!("native Source backing changed during exclusive retirement")
                });
        }
        self.base_registry = registry;
        self.advertisement = advertisement;
        self.kernel_resources = ledger;
        Ok(())
    }
}
