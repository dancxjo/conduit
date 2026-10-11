//! Independent network possession alongside the sole original USB owner.
//! This composes selected ordinary operations; no class request or packet policy.
use super::*;
use crate::ethernet_frame::{
    contract::EthernetFrameContract, factory::FRAME_IMPLEMENTATION,
    installation::EthernetFrameSelection, owned_installation::OwnedReadyEthernetFrameBase,
};
use smoltcp::time::Instant;

pub(crate) struct EthernetFrameAdmission {
    pub ready: OwnedReadyEthernetFrameBase,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
    pub random_seed: u64,
    pub timestamp: Instant,
}

impl<P: I2cProvider> PreparedProtocolCalls<P> {
    pub(crate) fn prepare_usb_ethernet(
        plan: &Plan,
        usb: crate::protocol_play::UsbDuplexAdmission,
        network: EthernetFrameAdmission,
        zip: &crate::flow_zip::FlowZipOperationFactory,
        states: &crate::seeded_state::SeededStateOperationFactory,
    ) -> Result<Self, ProtocolCallRefusal> {
        Self::prepare_usb_duplex_with_bindings(
            plan,
            usb,
            zip,
            states,
            &[FRAME_IMPLEMENTATION],
            |fragment, lowered, active, bindings| {
                let mut gears = fragment
                    .placements
                    .iter()
                    .filter(|gear| gear.implementation_id.as_str() == FRAME_IMPLEMENTATION);
                let gear = gears.next().ok_or(ProtocolCallRefusal::InvalidPlan)?;
                if gears.next().is_some() {
                    return Err(ProtocolCallRefusal::InvalidPlan);
                }
                let contract = EthernetFrameContract::prepare()
                    .map_err(|_| ProtocolCallRefusal::InvalidPlan)?;
                let owner = network
                    .ready
                    .bind_selected(
                        network.table,
                        network.handle,
                        network.claim,
                        &contract,
                        EthernetFrameSelection {
                            fragment,
                            lowered,
                            active,
                            placement: &gear.placement_id,
                        },
                        network.random_seed,
                        network.timestamp,
                    )
                    .map_err(ProtocolCallRefusal::EthernetFrame)?;
                bindings.push(Binding {
                    node: lowered
                        .identity
                        .node_for_placement(&gear.placement_id)
                        .ok_or(ProtocolCallRefusal::InvalidPlan)?,
                    resources: gear.resources.clone(),
                    authorities: gear.authority.clone(),
                    owner: Owner::EthernetFrame(Box::new(owner)),
                });
                Ok(())
            },
        )
    }

    /// Root supplies an actual admitted timestamp for one existing stack step.
    /// Calls never manufacture time or privately spin/retry the USB class graph.
    pub(crate) fn poll_ethernet_network(
        &mut self,
        timestamp: Instant,
    ) -> Result<Option<crate::virtio_tcp::VirtioTcpReceipt>, ProtocolCallRefusal> {
        let mut owners = self
            .bindings
            .iter_mut()
            .filter_map(|binding| match &mut binding.owner {
                Owner::EthernetFrame(owner) => Some(owner),
                _ => None,
            });
        let owner = owners.next().ok_or(ProtocolCallRefusal::InvalidPlan)?;
        if owners.next().is_some() {
            return Err(ProtocolCallRefusal::InvalidPlan);
        }
        owner
            .poll_network(timestamp)
            .map_err(ProtocolCallRefusal::EthernetFrame)
    }
}
