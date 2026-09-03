//! Finite browser device-Base admission. Browser APIs remain in the Host adapter.

use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BaseImplementationId, BaseInstanceId, BootId,
    CapabilityId, DeviceAssociation, HostId, HostOperationContractId, HostOperationId,
    OfferGeneration, PlanId, ResourceClassId, ResourceHandleId,
};

mod abi;
mod device_projection;
mod evidence;
mod transfer;
mod web_usb;

pub(crate) const SERIAL_ACQUIRE_OPERATION: &str = "conduit.host/acquire-web-serial@1";
pub(crate) const SERIAL_REQUEST_AUTHORITY: &str = "conduit.authority/request-web-serial@1";
pub(crate) const SERIAL_USE_AUTHORITY: &str = "conduit.authority/use-web-serial@1";
pub(crate) const SERIAL_RESOURCE_CLASS: &str = "conduit.resource/web-serial-port@1";
pub(crate) const SERIAL_BASE_IMPLEMENTATION: &str = "browser/web-serial@1";
pub(crate) const SERIAL_ACQUISITION_CAPABILITY: &str = "device/acquire-webserial@1";
pub(crate) const MAXIMUM_SERIAL_RESULT_BYTES: usize = 2_048;
pub(crate) const MAXIMUM_SERIAL_TRANSFER_BYTES: usize = 4_096;
pub(crate) const MAXIMUM_SERIAL_TRANSFERS: u16 = 40_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SerialParity {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SerialConfiguration {
    pub(crate) baud_rate: u32,
    pub(crate) data_bits: u8,
    pub(crate) stop_bits: u8,
    pub(crate) parity: SerialParity,
    pub(crate) buffer_size: u32,
}

impl SerialConfiguration {
    fn is_valid(self) -> bool {
        (1..=12_000_000).contains(&self.baud_rate)
            && matches!(self.data_bits, 7 | 8)
            && matches!(self.stop_bits, 1 | 2)
            && (1..=65_536).contains(&self.buffer_size)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SerialTransferBounds {
    pub(crate) maximum_transfer_bytes: u32,
    pub(crate) maximum_reads: u16,
    pub(crate) maximum_writes: u16,
    pub(crate) maximum_signal_operations: u16,
    pub(crate) maximum_in_flight: u8,
}

impl SerialTransferBounds {
    fn is_valid(self) -> bool {
        self.maximum_transfer_bytes > 0
            && self.maximum_transfer_bytes <= MAXIMUM_SERIAL_TRANSFER_BYTES as u32
            && self.maximum_reads > 0
            && self.maximum_reads <= MAXIMUM_SERIAL_TRANSFERS
            && self.maximum_writes > 0
            && self.maximum_writes <= MAXIMUM_SERIAL_TRANSFERS
            && self.maximum_signal_operations <= MAXIMUM_SERIAL_TRANSFERS
            && self.maximum_in_flight == 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialAcquisitionOffer {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) offer_generation: OfferGeneration,
    pub(crate) operation_contract: HostOperationContractId,
    pub(crate) request_authority_contract: AuthorityContractId,
    pub(crate) maximum_in_flight: u8,
    pub(crate) maximum_result_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialAcquisitionAuthority {
    pub(crate) grant_id: AuthorityGrantId,
    pub(crate) contract_id: AuthorityContractId,
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialAcquisitionRequest {
    pub(crate) operation_id: HostOperationId,
    pub(crate) configuration: SerialConfiguration,
    pub(crate) transfer_bounds: SerialTransferBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcquiredSerialResource {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) offer_generation: OfferGeneration,
    pub(crate) handle_id: ResourceHandleId,
    pub(crate) class_id: ResourceClassId,
    pub(crate) base_implementation_id: BaseImplementationId,
    pub(crate) base_instance_id: BaseInstanceId,
    pub(crate) configuration: SerialConfiguration,
    pub(crate) transfer_bounds: SerialTransferBounds,
    pub(crate) use_authority_contract: AuthorityContractId,
    pub(crate) use_authority_grant: AuthorityGrantId,
    pub(crate) usb_vendor_id: Option<u16>,
    pub(crate) usb_product_id: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SerialAcquisitionResult {
    Acquired(Box<AcquiredSerialResource>),
    PermissionDenied,
    NoPortSelected,
    Unsupported,
    OpenFailed,
    Cancelled,
    PlatformFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SerialUseRequirement {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) class_id: ResourceClassId,
    pub(crate) base_implementation_id: BaseImplementationId,
    pub(crate) base_instance_id: BaseInstanceId,
    pub(crate) transfer_bounds: SerialTransferBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SerialTransferDirection {
    Read,
    Write,
    Signals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserSerialTerminal {
    PermissionDenied,
    NoPortSelected,
    Unsupported,
    OpenFailed,
    AcquisitionCancelled,
    PlatformFailure,
    MalformedCompletion,
    ResourceCancelled,
    UseCancelled,
    DeviceLost,
    TransferTooLarge,
    TransferFailed,
    ReadClosed,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowserSerialPhase {
    OfferAvailable,
    AcquisitionPlanned {
        plan_id: PlanId,
        request: SerialAcquisitionRequest,
    },
    AcquisitionPlaying {
        plan_id: PlanId,
        request: SerialAcquisitionRequest,
    },
    ResourceTruth(AcquiredSerialResource),
    UsePlanned {
        plan_id: PlanId,
        resource: AcquiredSerialResource,
    },
    UsePlaying {
        plan_id: PlanId,
        resource: AcquiredSerialResource,
    },
    Terminal(BrowserSerialTerminal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserSerialRefusal {
    WrongPhase,
    InvalidOffer,
    RequestAuthorityMissing,
    RequestAuthorityMismatch,
    InvalidConfiguration,
    InvalidBounds,
    CompletionOperationMismatch,
    CompletionTooLarge,
    MalformedResource,
    UseAuthorityMissing,
    UseRequirementMismatch,
    TransferTooLarge,
    Pressure,
    TransferLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowserSerialSession {
    phase: BrowserSerialPhase,
    expected_operation: Option<HostOperationId>,
    expected_host_id: Option<HostId>,
    expected_boot_id: Option<BootId>,
    expected_offer_generation: Option<OfferGeneration>,
    retained_transfer: Option<(SerialTransferDirection, usize)>,
    admitted_reads: u16,
    admitted_writes: u16,
    admitted_signal_operations: u16,
}

impl BrowserSerialSession {
    pub(crate) const fn new() -> Self {
        Self {
            phase: BrowserSerialPhase::OfferAvailable,
            expected_operation: None,
            expected_host_id: None,
            expected_boot_id: None,
            expected_offer_generation: None,
            retained_transfer: None,
            admitted_reads: 0,
            admitted_writes: 0,
            admitted_signal_operations: 0,
        }
    }

    pub(crate) fn phase(&self) -> &BrowserSerialPhase {
        &self.phase
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        match self.retained_transfer {
            Some((_, bytes)) => bytes,
            None => 0,
        }
    }

    pub(crate) const fn admitted_reads(&self) -> u16 {
        self.admitted_reads
    }

    pub(crate) const fn admitted_writes(&self) -> u16 {
        self.admitted_writes
    }

    pub(crate) const fn admitted_signal_operations(&self) -> u16 {
        self.admitted_signal_operations
    }

    /// Projects current acquired-resource truth into optional Device context.
    /// Before acquisition and after loss/closure there is no current Device.
    pub(crate) fn current_device_association(
        &self,
        capability_ids: Vec<CapabilityId>,
    ) -> Option<DeviceAssociation> {
        device_projection::current_device_association(&self.phase, capability_ids)
    }

    pub(crate) fn seal_acquisition(
        &mut self,
        plan_id: PlanId,
        offer: &SerialAcquisitionOffer,
        authority: Option<&SerialAcquisitionAuthority>,
        request: SerialAcquisitionRequest,
    ) -> Result<(), BrowserSerialRefusal> {
        if !matches!(self.phase, BrowserSerialPhase::OfferAvailable) {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        if offer.operation_contract.as_str() != SERIAL_ACQUIRE_OPERATION
            || offer.request_authority_contract.as_str() != SERIAL_REQUEST_AUTHORITY
            || offer.maximum_in_flight != 1
            || offer.maximum_result_bytes as usize != MAXIMUM_SERIAL_RESULT_BYTES
        {
            return Err(BrowserSerialRefusal::InvalidOffer);
        }
        let authority = authority.ok_or(BrowserSerialRefusal::RequestAuthorityMissing)?;
        if authority.contract_id != offer.request_authority_contract
            || authority.host_id != offer.host_id
            || authority.boot_id != offer.boot_id
            || authority.grant_id.as_str().is_empty()
        {
            return Err(BrowserSerialRefusal::RequestAuthorityMismatch);
        }
        if !request.configuration.is_valid() {
            return Err(BrowserSerialRefusal::InvalidConfiguration);
        }
        if !request.transfer_bounds.is_valid() {
            return Err(BrowserSerialRefusal::InvalidBounds);
        }
        self.expected_operation = Some(request.operation_id.clone());
        self.expected_host_id = Some(offer.host_id.clone());
        self.expected_boot_id = Some(offer.boot_id.clone());
        self.expected_offer_generation = Some(offer.offer_generation);
        self.phase = BrowserSerialPhase::AcquisitionPlanned { plan_id, request };
        Ok(())
    }

    pub(crate) fn start_acquisition(&mut self) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::AcquisitionPlanned { plan_id, request } = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        self.phase = BrowserSerialPhase::AcquisitionPlaying {
            plan_id: plan_id.clone(),
            request: request.clone(),
        };
        Ok(())
    }

    pub(crate) fn complete_acquisition(
        &mut self,
        operation: &HostOperationId,
        encoded_result_bytes: usize,
        result: SerialAcquisitionResult,
    ) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::AcquisitionPlaying { request, .. } = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        if self.expected_operation.as_ref() != Some(operation) {
            return Err(BrowserSerialRefusal::CompletionOperationMismatch);
        }
        if encoded_result_bytes == 0 || encoded_result_bytes > MAXIMUM_SERIAL_RESULT_BYTES {
            self.phase = BrowserSerialPhase::Terminal(BrowserSerialTerminal::MalformedCompletion);
            self.expected_operation = None;
            return Err(BrowserSerialRefusal::CompletionTooLarge);
        }
        if let SerialAcquisitionResult::Acquired(resource) = &result {
            if resource.host_id.as_str().is_empty()
                || resource.boot_id.as_str().is_empty()
                || self.expected_host_id.as_ref() != Some(&resource.host_id)
                || self.expected_boot_id.as_ref() != Some(&resource.boot_id)
                || self.expected_offer_generation != Some(resource.offer_generation)
                || resource.handle_id.as_str().is_empty()
                || resource.class_id.as_str() != SERIAL_RESOURCE_CLASS
                || resource.base_implementation_id.as_str() != SERIAL_BASE_IMPLEMENTATION
                || resource.base_instance_id.as_str().is_empty()
                || resource.configuration != request.configuration
                || resource.transfer_bounds != request.transfer_bounds
                || resource.use_authority_contract.as_str() != SERIAL_USE_AUTHORITY
                || resource.use_authority_grant.as_str().is_empty()
            {
                self.phase =
                    BrowserSerialPhase::Terminal(BrowserSerialTerminal::MalformedCompletion);
                self.expected_operation = None;
                return Err(BrowserSerialRefusal::MalformedResource);
            }
        }
        self.expected_operation = None;
        self.expected_host_id = None;
        self.expected_boot_id = None;
        self.expected_offer_generation = None;
        self.phase = match result {
            SerialAcquisitionResult::Acquired(resource) => {
                BrowserSerialPhase::ResourceTruth(*resource)
            }
            SerialAcquisitionResult::PermissionDenied => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::PermissionDenied)
            }
            SerialAcquisitionResult::NoPortSelected => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::NoPortSelected)
            }
            SerialAcquisitionResult::Unsupported => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::Unsupported)
            }
            SerialAcquisitionResult::OpenFailed => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::OpenFailed)
            }
            SerialAcquisitionResult::Cancelled => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::AcquisitionCancelled)
            }
            SerialAcquisitionResult::PlatformFailure => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::PlatformFailure)
            }
        };
        Ok(())
    }

    pub(crate) fn seal_use(
        &mut self,
        plan_id: PlanId,
        requirement: &SerialUseRequirement,
        authority: Option<&AuthorityGrantId>,
    ) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::ResourceTruth(resource) = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        if authority != Some(&resource.use_authority_grant) {
            return Err(BrowserSerialRefusal::UseAuthorityMissing);
        }
        if requirement.host_id != resource.host_id
            || requirement.boot_id != resource.boot_id
            || requirement.class_id != resource.class_id
            || requirement.base_implementation_id != resource.base_implementation_id
            || requirement.base_instance_id != resource.base_instance_id
            || requirement.transfer_bounds != resource.transfer_bounds
        {
            return Err(BrowserSerialRefusal::UseRequirementMismatch);
        }
        self.phase = BrowserSerialPhase::UsePlanned {
            plan_id,
            resource: resource.clone(),
        };
        Ok(())
    }

    pub(crate) fn start_use(&mut self) -> Result<(), BrowserSerialRefusal> {
        let BrowserSerialPhase::UsePlanned { plan_id, resource } = &self.phase else {
            return Err(BrowserSerialRefusal::WrongPhase);
        };
        self.phase = BrowserSerialPhase::UsePlaying {
            plan_id: plan_id.clone(),
            resource: resource.clone(),
        };
        Ok(())
    }

    pub(crate) fn device_lost(&mut self) -> Result<(), BrowserSerialRefusal> {
        if !matches!(
            self.phase,
            BrowserSerialPhase::ResourceTruth(_)
                | BrowserSerialPhase::UsePlanned { .. }
                | BrowserSerialPhase::UsePlaying { .. }
        ) {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        self.retained_transfer = None;
        self.phase = BrowserSerialPhase::Terminal(BrowserSerialTerminal::DeviceLost);
        Ok(())
    }

    pub(crate) fn close(&mut self) -> Result<(), BrowserSerialRefusal> {
        if !matches!(
            self.phase,
            BrowserSerialPhase::ResourceTruth(_)
                | BrowserSerialPhase::UsePlanned { .. }
                | BrowserSerialPhase::UsePlaying { .. }
        ) {
            return Err(BrowserSerialRefusal::WrongPhase);
        }
        self.retained_transfer = None;
        self.phase = BrowserSerialPhase::Terminal(BrowserSerialTerminal::Closed);
        Ok(())
    }

    pub(crate) fn cancel(&mut self) -> Result<(), BrowserSerialRefusal> {
        self.expected_operation = None;
        self.expected_host_id = None;
        self.expected_boot_id = None;
        self.expected_offer_generation = None;
        self.retained_transfer = None;
        self.phase = match self.phase {
            BrowserSerialPhase::AcquisitionPlanned { .. }
            | BrowserSerialPhase::AcquisitionPlaying { .. } => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::AcquisitionCancelled)
            }
            BrowserSerialPhase::ResourceTruth(_) => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::ResourceCancelled)
            }
            BrowserSerialPhase::UsePlanned { .. } | BrowserSerialPhase::UsePlaying { .. } => {
                BrowserSerialPhase::Terminal(BrowserSerialTerminal::UseCancelled)
            }
            _ => return Err(BrowserSerialRefusal::WrongPhase),
        };
        Ok(())
    }
}

impl Default for BrowserSerialSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
