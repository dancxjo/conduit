//! Endpoint-only admission keeps the sole original owner without a control lease.
use super::*;

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(crate) fn prepare_usb_endpoint_only(
        plan: &Plan,
        admission: crate::protocol_play::UsbEndpointAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        use crate::usb_base::endpoint_read_factory::ENDPOINT_READ_IMPLEMENTATION;
        use ProtocolCallRefusal as Refusal;
        let (lowered, active, mut bindings) =
            Self::prepare_pure(plan, &[ENDPOINT_READ_IMPLEMENTATION], zip, states)?;
        let fragment = &plan.fragments[0];
        let endpoint = fragment
            .placements
            .iter()
            .find(|gear| gear.implementation_id.as_str() == ENDPOINT_READ_IMPLEMENTATION)
            .ok_or(Refusal::Unsupported)?;
        let contract = crate::usb_base::endpoint_read_contract::EndpointReadContract::prepare()
            .map_err(|_| Refusal::InvalidPlan)?;
        let host = admission
            .ready
            .bind_endpoint_selected(
                admission.endpoint_table,
                admission.endpoint_handle,
                admission.endpoint_claim,
                &contract,
                crate::machine_membrane::selected_operation::SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &endpoint.placement_id,
                },
            )
            .map_err(Refusal::UsbProtocol)?;
        bindings.push(Binding {
            node: lowered
                .identity
                .node_for_placement(&endpoint.placement_id)
                .ok_or(Refusal::InvalidPlan)?,
            resources: endpoint.resources.clone(),
            authorities: endpoint.authority.clone(),
            owner: Owner::UsbProtocolEndpoint,
        });
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            storage: None,
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
}
