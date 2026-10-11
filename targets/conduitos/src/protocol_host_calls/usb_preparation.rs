//! Preparation of independently admitted USB operations on one native owner.
use super::{
    Binding, I2cProvider, Owner, PendingUsbKind, PreparedProtocolCalls, ProtocolCallRefusal,
};
use alloc::boxed::Box;
use conduit_core::*;

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb(
        plan: &Plan,
        ready: crate::arch::ReadyUsbControlBase,
        table: BaseCapabilityTable,
        handle: BaseCapabilityHandle,
        claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_selected(
            plan,
            crate::usb_base::control_factory::CONTROL_IMPLEMENTATION,
            zip,
            states,
            |fragment, lowered, active, placement| {
                let contract = crate::usb_base::control_contract::ControlContract::prepare()
                    .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                ready
                    .bind_selected(
                        table,
                        handle,
                        claim,
                        crate::arch::UsbControlSelection {
                            contract: &contract,
                            fragment,
                            lowered,
                            active,
                            placement,
                        },
                    )
                    .map(|owner| Owner::UsbControl(Box::new(owner)))
                    .map_err(ProtocolCallRefusal::Usb)
            },
        )
    }

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_usb_protocol(
        plan: &Plan,
        ready: crate::arch::ReadyUsbProtocolHost,
        control_table: BaseCapabilityTable,
        control_handle: BaseCapabilityHandle,
        control_claim: BaseOperationClaim,
        endpoint_table: BaseCapabilityTable,
        endpoint_handle: BaseCapabilityHandle,
        endpoint_claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        use crate::usb_base::{
            control_factory::CONTROL_IMPLEMENTATION,
            endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
        };
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) = Self::prepare_pure(
            plan,
            &[CONTROL_IMPLEMENTATION, ENDPOINT_READ_IMPLEMENTATION],
            zip,
            states,
        )?;
        let fragment = &plan.fragments[0];
        let control = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let control_contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(|_| Refusal::InvalidPlan)?;
        let endpoint_contract =
            crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
                .map_err(|_| Refusal::InvalidPlan)?;
        let host = ready
            .bind_selected(
                control_table,
                control_handle,
                control_claim,
                crate::arch::UsbControlSelection {
                    contract: &control_contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &control.placement_id,
                },
                endpoint_table,
                endpoint_handle,
                endpoint_claim,
                &endpoint_contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &endpoint.placement_id,
                },
            )
            .map_err(Refusal::UsbProtocol)?;
        for (gear, owner) in [
            (control, Owner::UsbProtocolControl),
            (endpoint, Owner::UsbProtocolEndpoint),
        ] {
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner,
            });
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            usb_protocol: Some(host),
            usb_duplex: None,
            usb_isochronous: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_kind: PendingUsbKind::ProtocolRead,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_failed: false,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_duplex: [None, None],
        })
    }

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare_usb_isochronous(
        plan: &Plan,
        ready: crate::arch::ReadyUsbIsochronousHost,
        control_table: BaseCapabilityTable,
        control_handle: BaseCapabilityHandle,
        control_claim: BaseOperationClaim,
        endpoint_table: BaseCapabilityTable,
        endpoint_handle: BaseCapabilityHandle,
        endpoint_claim: BaseOperationClaim,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        use crate::usb_base::{
            control_factory::CONTROL_IMPLEMENTATION,
            isochronous::factory::ISOCHRONOUS_OUT_IMPLEMENTATION,
        };
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) = Self::prepare_pure(
            plan,
            &[CONTROL_IMPLEMENTATION, ISOCHRONOUS_OUT_IMPLEMENTATION],
            zip,
            states,
        )?;
        let fragment = &plan.fragments[0];
        let control = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ISOCHRONOUS_OUT_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let control_contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(|_| Refusal::InvalidPlan)?;
        let endpoint_contract =
            crate::usb_base::isochronous::contract::IsochronousOutContract::prepare()
                .map_err(|_| Refusal::InvalidPlan)?;
        let host = ready
            .bind_selected(
                control_table,
                control_handle,
                control_claim,
                crate::arch::UsbControlSelection {
                    contract: &control_contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &control.placement_id,
                },
                endpoint_table,
                endpoint_handle,
                endpoint_claim,
                &endpoint_contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &endpoint.placement_id,
                },
            )
            .map_err(Refusal::UsbIsochronous)?;
        for (gear, owner) in [
            (control, Owner::UsbIsochronousControl),
            (endpoint, Owner::UsbIsochronousOut),
        ] {
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner,
            });
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            usb_protocol: None,
            usb_isochronous: Some(host),
            usb_duplex: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_kind: PendingUsbKind::ProtocolRead,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_failed: false,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_duplex: [None, None],
        })
    }

    #[cfg(any(
        target_arch = "x86_64",
        all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
    ))]
    pub(crate) fn prepare_usb_duplex(
        plan: &Plan,
        admission: crate::protocol_play::UsbDuplexAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        use crate::usb_base::{
            control_factory::CONTROL_IMPLEMENTATION,
            endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION,
            endpoint_write_factory::ENDPOINT_WRITE_IMPLEMENTATION,
        };
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) = Self::prepare_pure(
            plan,
            &[
                CONTROL_IMPLEMENTATION,
                ENDPOINT_READ_IMPLEMENTATION,
                ENDPOINT_WRITE_IMPLEMENTATION,
            ],
            zip,
            states,
        )?;
        let fragment = &plan.fragments[0];
        let control = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == CONTROL_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let write = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ENDPOINT_WRITE_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let write_contract =
            crate::usb_base::endpoint_write_contract::EndpointWriteContract::prepare()
                .map_err(|_| Refusal::InvalidPlan)?;
        let control_contract = crate::usb_base::control_contract::ControlContract::prepare()
            .map_err(|_| Refusal::InvalidPlan)?;
        let endpoint_contract =
            crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
                .map_err(|_| Refusal::InvalidPlan)?;
        let host = admission
            .ready
            .bind_selected(
                admission.control_table,
                admission.control_handle,
                admission.control_claim,
                crate::arch::UsbControlSelection {
                    contract: &control_contract,
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement: &control.placement_id,
                },
                admission.endpoint_table,
                admission.endpoint_handle,
                admission.endpoint_claim,
                &endpoint_contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &endpoint.placement_id,
                },
                admission.write_table,
                admission.write_handle,
                admission.write_claim,
                &write_contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &write.placement_id,
                },
            )
            .map_err(Refusal::UsbProtocol)?;
        for (gear, owner) in [
            (control, Owner::UsbDuplexControl),
            (endpoint, Owner::UsbDuplexRead),
            (write, Owner::UsbDuplexWrite),
        ] {
            bindings.push(Binding {
                node: lowered
                    .identity
                    .node_for_placement(&gear.placement_id)
                    .ok_or(Refusal::InvalidPlan)?,
                resources: gear.resources.clone(),
                authorities: gear.authority.clone(),
                owner,
            });
        }
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            identity: PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            bindings,
            usb_protocol: None,
            usb_duplex: Some(host),
            usb_isochronous: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_kind: PendingUsbKind::ProtocolRead,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb: None,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_usb_failed: false,
            #[cfg(any(
                target_arch = "x86_64",
                all(target_arch = "aarch64", not(feature = "aarch64-orange-pi-5"))
            ))]
            pending_duplex: [None, None],
        })
    }
}
