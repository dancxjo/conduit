//! USB storage admission preserves the ordinary shared kernel and exact owners.
use super::*;

impl<P: I2cProvider> PreparedProtocolPlay<P> {
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub fn prepare_usb(
        definition: KernelCompositeDefinition,
        ready: crate::arch::ReadyUsbControlBase,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_usb_with_sign_storage(definition, ready, table, handle, claim, operations, 0)
    }

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb_with_sign_storage(
        definition: KernelCompositeDefinition,
        ready: crate::arch::ReadyUsbControlBase,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        operations: crate::protocol_operations::ProtocolOperations,
        additional_remote_sign_items: u16,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            definition,
            operations,
            false,
            PhysicalMode::UsbControl,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 0,
                additional_remote_items: additional_remote_sign_items,
            },
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_usb(plan, ready, table, handle, claim, zip, states)
            },
        )
    }

    /// Both exact native operations retain one controller and ordinary kernel.
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb_protocol(
        definition: KernelCompositeDefinition,
        admission: UsbProtocolAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            definition,
            operations,
            false,
            PhysicalMode::UsbProtocol,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 0,
                additional_remote_items: admission.additional_remote_sign_items,
            },
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_usb_protocol(
                    plan,
                    admission.ready,
                    admission.control_table,
                    admission.control_handle,
                    admission.control_claim,
                    admission.endpoint_table,
                    admission.endpoint_handle,
                    admission.endpoint_claim,
                    zip,
                    states,
                )
            },
        )
    }

    /// Both exact native operations retain one controller and ordinary kernel.
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb_isochronous(
        definition: KernelCompositeDefinition,
        admission: UsbIsochronousAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            definition,
            operations,
            false,
            PhysicalMode::UsbIsochronous,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 0,
                additional_remote_items: admission.additional_remote_sign_items,
            },
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_usb_isochronous(
                    plan,
                    admission.ready,
                    admission.control_table,
                    admission.control_handle,
                    admission.control_claim,
                    admission.endpoint_table,
                    admission.endpoint_handle,
                    admission.endpoint_claim,
                    zip,
                    states,
                )
            },
        )
    }

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb_duplex(
        definition: KernelCompositeDefinition,
        admission: UsbDuplexAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            definition,
            operations,
            false,
            PhysicalMode::UsbDuplex,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 0,
                additional_remote_items: admission.additional_remote_sign_items,
            },
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_usb_duplex(plan, admission, zip, states)
            },
        )
    }
}

/// Actual independently issued storage possessions; descriptive offers are separate.
pub(crate) struct StorageScopePossession {
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}
pub(crate) struct StorageUsbAdmission {
    pub usb: UsbDuplexAdmission,
    pub authorize: StorageScopePossession,
    pub settle: StorageScopePossession,
}
impl<P: I2cProvider> PreparedProtocolPlay<P> {
    pub(crate) fn prepare_storage_usb_duplex(
        definition: KernelCompositeDefinition,
        admission: StorageUsbAdmission,
        operations: crate::protocol_operations::ProtocolOperations,
    ) -> Result<Self, ProtocolCallRefusal> {
        // Ordinary ingress cannot submit raw USB calls alongside storage requests.
        let plan = &definition.internal_plan;
        let authorize = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .find(|gear| {
                gear.implementation_id.as_str()
                    == crate::storage_base::transaction_factory::AUTHORIZE_IMPLEMENTATION
            })
            .ok_or(ProtocolCallRefusal::InvalidPlan)?;
        if definition.boundary.input_fronts.len() != 1
            || definition.boundary.input_fronts[0].internal_placement_id != authorize.placement_id
        {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        Self::prepare_selected(
            definition,
            operations,
            false,
            PhysicalMode::StorageUsbDuplex,
            conduit_composite::KernelCompositeSignStorage {
                additional_local_items: 0,
                additional_remote_items: admission.usb.additional_remote_sign_items,
            },
            |plan, zip, states| {
                PreparedProtocolCalls::prepare_storage_usb_duplex(plan, admission, zip, states)
            },
        )
    }
}
