//! Finite WebUSB Base admission. The sibling browser adapter owns only WebUSB mechanics.

use conduit_core::{
    AuthorityContractId, AuthorityGrantId, BaseImplementationId, BaseInstanceId, BootId, HostId,
    HostOperationContractId, HostOperationId, OfferGeneration, PlanId, ResourceClassId,
    ResourceHandleId,
};

mod abi;
mod evidence;
mod transfer;

pub(crate) const USB_ACQUIRE_OPERATION: &str = "conduit.host/acquire-web-usb@1";
pub(crate) const USB_REQUEST_AUTHORITY: &str = "conduit.authority/request-web-usb@1";
pub(crate) const USB_USE_AUTHORITY: &str = "conduit.authority/use-web-usb@1";
pub(crate) const USB_RESOURCE_CLASS: &str = "conduit.resource/web-usb-device@1";
pub(crate) const USB_BASE_IMPLEMENTATION: &str = "browser/web-usb@1";
pub(crate) const MAXIMUM_USB_RESULT_BYTES: usize = 2_048;
pub(crate) const MAXIMUM_USB_TRANSFER_BYTES: usize = 4_096;
pub(crate) const MAXIMUM_USB_TRANSFERS: u16 = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UsbConfiguration {
    pub(crate) configuration_value: u8,
    pub(crate) interface_number: u8,
    pub(crate) alternate_setting: u8,
    pub(crate) in_endpoint: u8,
    pub(crate) out_endpoint: u8,
}

impl UsbConfiguration {
    fn is_valid(self) -> bool {
        self.configuration_value > 0
            && (1..=15).contains(&self.in_endpoint)
            && (1..=15).contains(&self.out_endpoint)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UsbTransferBounds {
    pub(crate) maximum_transfer_bytes: u32,
    pub(crate) maximum_in_transfers: u16,
    pub(crate) maximum_out_transfers: u16,
    pub(crate) maximum_in_flight: u8,
}

impl UsbTransferBounds {
    fn is_valid(self) -> bool {
        self.maximum_transfer_bytes > 0
            && self.maximum_transfer_bytes <= MAXIMUM_USB_TRANSFER_BYTES as u32
            && self.maximum_in_transfers > 0
            && self.maximum_in_transfers <= MAXIMUM_USB_TRANSFERS
            && self.maximum_out_transfers > 0
            && self.maximum_out_transfers <= MAXIMUM_USB_TRANSFERS
            && self.maximum_in_flight == 1
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsbAcquisitionOffer {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) offer_generation: OfferGeneration,
    pub(crate) operation_contract: HostOperationContractId,
    pub(crate) request_authority_contract: AuthorityContractId,
    pub(crate) maximum_in_flight: u8,
    pub(crate) maximum_result_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsbAcquisitionAuthority {
    pub(crate) grant_id: AuthorityGrantId,
    pub(crate) contract_id: AuthorityContractId,
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsbAcquisitionRequest {
    pub(crate) operation_id: HostOperationId,
    pub(crate) configuration: UsbConfiguration,
    pub(crate) transfer_bounds: UsbTransferBounds,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcquiredUsbResource {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) handle_id: ResourceHandleId,
    pub(crate) class_id: ResourceClassId,
    pub(crate) base_implementation_id: BaseImplementationId,
    pub(crate) base_instance_id: BaseInstanceId,
    pub(crate) configuration: UsbConfiguration,
    pub(crate) transfer_bounds: UsbTransferBounds,
    pub(crate) use_authority_contract: AuthorityContractId,
    pub(crate) use_authority_grant: AuthorityGrantId,
    pub(crate) vendor_id: u16,
    pub(crate) product_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum UsbAcquisitionResult {
    Acquired(Box<AcquiredUsbResource>),
    PermissionDenied,
    NoDeviceSelected,
    Unsupported,
    OpenFailed,
    ConfigurationFailed,
    InterfaceClaimFailed,
    AlternateFailed,
    Cancelled,
    PlatformFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UsbUseRequirement {
    pub(crate) host_id: HostId,
    pub(crate) boot_id: BootId,
    pub(crate) class_id: ResourceClassId,
    pub(crate) base_implementation_id: BaseImplementationId,
    pub(crate) base_instance_id: BaseInstanceId,
    pub(crate) configuration: UsbConfiguration,
    pub(crate) transfer_bounds: UsbTransferBounds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbTransferDirection {
    In,
    Out,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbTransferKind {
    Bulk,
    Control,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbControlRequestType {
    Standard,
    Class,
    Vendor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UsbControlRecipient {
    Device,
    Interface,
    Endpoint,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct UsbControlSetup {
    pub(crate) request_type: UsbControlRequestType,
    pub(crate) recipient: UsbControlRecipient,
    pub(crate) request: u8,
    pub(crate) value: u16,
    pub(crate) index: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RetainedUsbTransfer {
    kind: UsbTransferKind,
    direction: UsbTransferDirection,
    control_setup: Option<UsbControlSetup>,
    completed_bytes: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserUsbTerminal {
    PermissionDenied,
    NoDeviceSelected,
    Unsupported,
    OpenFailed,
    ConfigurationFailed,
    InterfaceClaimFailed,
    AlternateFailed,
    AcquisitionCancelled,
    PlatformFailure,
    MalformedCompletion,
    ResourceCancelled,
    UseCancelled,
    DeviceLost,
    TransferTooLarge,
    TransferStalled,
    TransferBabble,
    TransferFailed,
    CloseFailed,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BrowserUsbPhase {
    OfferAvailable,
    AcquisitionPlanned {
        plan_id: PlanId,
        request: UsbAcquisitionRequest,
    },
    AcquisitionPlaying {
        plan_id: PlanId,
        request: UsbAcquisitionRequest,
    },
    ResourceTruth(AcquiredUsbResource),
    UsePlanned {
        plan_id: PlanId,
        resource: AcquiredUsbResource,
    },
    UsePlaying {
        plan_id: PlanId,
        resource: AcquiredUsbResource,
    },
    Terminal(BrowserUsbTerminal),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserUsbRefusal {
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
pub(crate) struct BrowserUsbSession {
    phase: BrowserUsbPhase,
    expected_operation: Option<HostOperationId>,
    expected_host_id: Option<HostId>,
    expected_boot_id: Option<BootId>,
    retained_transfer: Option<RetainedUsbTransfer>,
    admitted_in_transfers: u16,
    admitted_out_transfers: u16,
}

impl BrowserUsbSession {
    pub(crate) const fn new() -> Self {
        Self {
            phase: BrowserUsbPhase::OfferAvailable,
            expected_operation: None,
            expected_host_id: None,
            expected_boot_id: None,
            retained_transfer: None,
            admitted_in_transfers: 0,
            admitted_out_transfers: 0,
        }
    }

    pub(crate) fn phase(&self) -> &BrowserUsbPhase {
        &self.phase
    }
    pub(crate) fn retained_bytes(&self) -> usize {
        self.retained_transfer
            .and_then(|transfer| transfer.completed_bytes)
            .unwrap_or(0)
    }
    pub(crate) fn retained_control_setup(&self) -> Option<UsbControlSetup> {
        self.retained_transfer
            .and_then(|transfer| transfer.control_setup)
    }
    pub(crate) const fn admitted_in_transfers(&self) -> u16 {
        self.admitted_in_transfers
    }
    pub(crate) const fn admitted_out_transfers(&self) -> u16 {
        self.admitted_out_transfers
    }

    pub(crate) fn seal_acquisition(
        &mut self,
        plan_id: PlanId,
        offer: &UsbAcquisitionOffer,
        authority: Option<&UsbAcquisitionAuthority>,
        request: UsbAcquisitionRequest,
    ) -> Result<(), BrowserUsbRefusal> {
        if !matches!(self.phase, BrowserUsbPhase::OfferAvailable) {
            return Err(BrowserUsbRefusal::WrongPhase);
        }
        if offer.operation_contract.as_str() != USB_ACQUIRE_OPERATION
            || offer.request_authority_contract.as_str() != USB_REQUEST_AUTHORITY
            || offer.maximum_in_flight != 1
            || offer.maximum_result_bytes as usize != MAXIMUM_USB_RESULT_BYTES
        {
            return Err(BrowserUsbRefusal::InvalidOffer);
        }
        let authority = authority.ok_or(BrowserUsbRefusal::RequestAuthorityMissing)?;
        if authority.contract_id != offer.request_authority_contract
            || authority.host_id != offer.host_id
            || authority.boot_id != offer.boot_id
            || authority.grant_id.as_str().is_empty()
        {
            return Err(BrowserUsbRefusal::RequestAuthorityMismatch);
        }
        if !request.configuration.is_valid() {
            return Err(BrowserUsbRefusal::InvalidConfiguration);
        }
        if !request.transfer_bounds.is_valid() {
            return Err(BrowserUsbRefusal::InvalidBounds);
        }
        self.expected_operation = Some(request.operation_id.clone());
        self.expected_host_id = Some(offer.host_id.clone());
        self.expected_boot_id = Some(offer.boot_id.clone());
        self.phase = BrowserUsbPhase::AcquisitionPlanned { plan_id, request };
        Ok(())
    }

    pub(crate) fn start_acquisition(&mut self) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::AcquisitionPlanned { plan_id, request } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        self.phase = BrowserUsbPhase::AcquisitionPlaying {
            plan_id: plan_id.clone(),
            request: request.clone(),
        };
        Ok(())
    }

    pub(crate) fn complete_acquisition(
        &mut self,
        operation: &HostOperationId,
        encoded_result_bytes: usize,
        result: UsbAcquisitionResult,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::AcquisitionPlaying { request, .. } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if self.expected_operation.as_ref() != Some(operation) {
            return Err(BrowserUsbRefusal::CompletionOperationMismatch);
        }
        if encoded_result_bytes == 0 || encoded_result_bytes > MAXIMUM_USB_RESULT_BYTES {
            self.phase = BrowserUsbPhase::Terminal(BrowserUsbTerminal::MalformedCompletion);
            self.expected_operation = None;
            return Err(BrowserUsbRefusal::CompletionTooLarge);
        }
        if let UsbAcquisitionResult::Acquired(resource) = &result {
            if self.expected_host_id.as_ref() != Some(&resource.host_id)
                || self.expected_boot_id.as_ref() != Some(&resource.boot_id)
                || resource.handle_id.as_str().is_empty()
                || resource.class_id.as_str() != USB_RESOURCE_CLASS
                || resource.base_implementation_id.as_str() != USB_BASE_IMPLEMENTATION
                || resource.base_instance_id.as_str().is_empty()
                || resource.configuration != request.configuration
                || resource.transfer_bounds != request.transfer_bounds
                || resource.use_authority_contract.as_str() != USB_USE_AUTHORITY
                || resource.use_authority_grant.as_str().is_empty()
            {
                self.phase = BrowserUsbPhase::Terminal(BrowserUsbTerminal::MalformedCompletion);
                self.expected_operation = None;
                return Err(BrowserUsbRefusal::MalformedResource);
            }
        }
        self.expected_operation = None;
        self.expected_host_id = None;
        self.expected_boot_id = None;
        self.phase = match result {
            UsbAcquisitionResult::Acquired(resource) => BrowserUsbPhase::ResourceTruth(*resource),
            UsbAcquisitionResult::PermissionDenied => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::PermissionDenied)
            }
            UsbAcquisitionResult::NoDeviceSelected => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::NoDeviceSelected)
            }
            UsbAcquisitionResult::Unsupported => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::Unsupported)
            }
            UsbAcquisitionResult::OpenFailed => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::OpenFailed)
            }
            UsbAcquisitionResult::ConfigurationFailed => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::ConfigurationFailed)
            }
            UsbAcquisitionResult::InterfaceClaimFailed => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::InterfaceClaimFailed)
            }
            UsbAcquisitionResult::AlternateFailed => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::AlternateFailed)
            }
            UsbAcquisitionResult::Cancelled => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::AcquisitionCancelled)
            }
            UsbAcquisitionResult::PlatformFailure => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::PlatformFailure)
            }
        };
        Ok(())
    }

    pub(crate) fn seal_use(
        &mut self,
        plan_id: PlanId,
        requirement: &UsbUseRequirement,
        authority: Option<&AuthorityGrantId>,
    ) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::ResourceTruth(resource) = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        if authority != Some(&resource.use_authority_grant) {
            return Err(BrowserUsbRefusal::UseAuthorityMissing);
        }
        if requirement.host_id != resource.host_id
            || requirement.boot_id != resource.boot_id
            || requirement.class_id != resource.class_id
            || requirement.base_implementation_id != resource.base_implementation_id
            || requirement.base_instance_id != resource.base_instance_id
            || requirement.configuration != resource.configuration
            || requirement.transfer_bounds != resource.transfer_bounds
        {
            return Err(BrowserUsbRefusal::UseRequirementMismatch);
        }
        self.phase = BrowserUsbPhase::UsePlanned {
            plan_id,
            resource: resource.clone(),
        };
        Ok(())
    }

    pub(crate) fn start_use(&mut self) -> Result<(), BrowserUsbRefusal> {
        let BrowserUsbPhase::UsePlanned { plan_id, resource } = &self.phase else {
            return Err(BrowserUsbRefusal::WrongPhase);
        };
        self.phase = BrowserUsbPhase::UsePlaying {
            plan_id: plan_id.clone(),
            resource: resource.clone(),
        };
        Ok(())
    }

    pub(crate) fn cancel(&mut self) -> Result<(), BrowserUsbRefusal> {
        self.expected_operation = None;
        self.expected_host_id = None;
        self.expected_boot_id = None;
        self.retained_transfer = None;
        self.phase = match self.phase {
            BrowserUsbPhase::AcquisitionPlanned { .. }
            | BrowserUsbPhase::AcquisitionPlaying { .. } => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::AcquisitionCancelled)
            }
            BrowserUsbPhase::ResourceTruth(_) => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::ResourceCancelled)
            }
            BrowserUsbPhase::UsePlanned { .. } | BrowserUsbPhase::UsePlaying { .. } => {
                BrowserUsbPhase::Terminal(BrowserUsbTerminal::UseCancelled)
            }
            _ => return Err(BrowserUsbRefusal::WrongPhase),
        };
        Ok(())
    }
}

impl Default for BrowserUsbSession {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
