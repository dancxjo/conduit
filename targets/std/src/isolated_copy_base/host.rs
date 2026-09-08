pub struct IsolatedFileHost {
    host: crate::StdHost,
    provider: crate::IsolatedFileBaseConfig,
}

/// Patchbay-safe realization truth. It deliberately contains neither OS paths
/// nor the private capability bearer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolatedFileBaseInspection {
    pub base_instance_id: conduit_core::BaseInstanceId,
    pub provider_generation: u64,
    pub implementation_id: conduit_core::BaseImplementationId,
    pub enforcement_class: conduit_core::BaseEnforcementClass,
    pub attempt_id: crate::CopyRequestId,
    pub source_resource: conduit_core::ResourceHandleId,
    pub destination_resource: conduit_core::ResourceHandleId,
    pub result: crate::CopyResult,
}

impl IsolatedFileHost {
    pub fn new(
        config: crate::StdHostConfig,
        mut composition: crate::StdHostComposition,
        provider: crate::IsolatedFileBaseConfig,
    ) -> Result<Self, String> {
        provider.validate()?;
        composition.files = true;
        let mut host = crate::StdHost::new_with_composition(config, composition);
        host.advertisement.capabilities.retain(|offer| {
            offer.implementation.implementation_id.as_str()
                != conduit_std_offers::COPY_FILE_IMPLEMENTATION
        });
        host.advertisement
            .capabilities
            .push(conduit_std_offers::isolated_copy_file_offer());
        host.kernel_resources =
            crate::kernel_preparation::KernelResourceLedger::new(&host.advertisement)
                .map_err(|error| format!("isolated file Base resources: {error}"))?;
        Ok(Self { host, provider })
    }

    pub fn host(&self) -> &crate::StdHost {
        &self.host
    }

    pub fn host_mut(&mut self) -> &mut crate::StdHost {
        &mut self.host
    }

    pub fn run_copy_fragment(
        &mut self,
        play: crate::IssuedKernelPlay,
        request_id: crate::CopyRequestId,
        fragment: conduit_core::PlanFragment,
        registry: &mut crate::ProtectedFileRegistry,
        stop: &crate::CopyStopToken,
    ) -> Result<crate::CopyRunReceipt, String> {
        self.host.run_copy_fragment_isolated(
            play,
            request_id,
            fragment,
            registry,
            stop,
            &self.provider,
        )
    }

    pub fn inspection(&self, receipt: &crate::CopyRunReceipt) -> IsolatedFileBaseInspection {
        IsolatedFileBaseInspection {
            base_instance_id: self.provider.base_instance_id.clone(),
            provider_generation: self.provider.provider_generation,
            implementation_id: conduit_core::BaseImplementationId::from(
                conduit_std_offers::ISOLATED_COPY_FILE_IMPLEMENTATION,
            ),
            enforcement_class: conduit_core::BaseEnforcementClass::ProcessIsolated,
            attempt_id: receipt.request_id.clone(),
            source_resource: receipt.source_binding_id.clone(),
            destination_resource: receipt.destination_binding_id.clone(),
            result: receipt.result.clone(),
        }
    }
}
