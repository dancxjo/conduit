//! Retained Plan C sink state shared by its admitted carrier candidates.

use conduit_core::{
    bind_active_play, BootId, ConnectionBase, ConnectionBaseInstanceId, ConnectionId, FragmentId,
    HostId, KindId, LinkBindingId, LinkEndpointId, LinkLimits, PlanId,
};
use conduit_wire::{
    RouteAttachment, SessionBinding, SessionCheckpointAcceptance, SessionCheckpointOffer,
    SessionEndpointIdentity, SessionLimits, SessionMachine, SessionRole,
};

use crate::receipts::RuntimeTranscriptIdentity;
use crate::remote_kernel::RemoteSignalKernel;
use crate::signal_execution_identity::SignalExecutionIdentity;
use crate::signal_image::RemoteEndpointIdentity;
use crate::usb_link::UsbLinkError;

pub struct ContinuableSignalSink {
    pub machine: SessionMachine,
    pub kernel: RemoteSignalKernel,
    pub identity: SignalExecutionIdentity,
}

impl ContinuableSignalSink {
    pub fn new_plan_a(runtime: &RuntimeTranscriptIdentity) -> Result<Self, UsbLinkError> {
        let identity = SignalExecutionIdentity::plan_a();
        let endpoint = crate::signal_image::generated_remote_endpoint()
            .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        Self::new_for(endpoint, identity, runtime)
    }

    pub fn new(runtime: &RuntimeTranscriptIdentity) -> Result<Self, UsbLinkError> {
        if !crate::plan_c_signal_image::validate() {
            return Err(UsbLinkError::InvalidGeneratedEndpoint);
        }
        let identity = crate::plan_c_signal_image::execution_identity();
        let endpoint = crate::plan_c_signal_image::endpoint(ConnectionBase::WebSocket)
            .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        Self::new_for(endpoint, identity, runtime)
    }

    fn new_for(
        endpoint: RemoteEndpointIdentity,
        identity: SignalExecutionIdentity,
        runtime: &RuntimeTranscriptIdentity,
    ) -> Result<Self, UsbLinkError> {
        let binding = binding(endpoint, identity, runtime)?;
        Ok(Self {
            machine: SessionMachine::new(binding, SessionRole::Sink)
                .map_err(UsbLinkError::Codec)?,
            kernel: RemoteSignalKernel::new_for_endpoint(identity, endpoint.endpoint, endpoint.cord)?,
            identity,
        })
    }

    pub fn binding(&self) -> &SessionBinding {
        self.machine.binding()
    }

    pub fn resume_usb(
        &mut self,
        runtime: &RuntimeTranscriptIdentity,
        peer: SessionCheckpointOffer<'_>,
    ) -> Result<SessionCheckpointAcceptance, UsbLinkError> {
        let endpoint = crate::plan_c_signal_image::endpoint(ConnectionBase::UsbCdc)
            .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
        let replacement = binding(endpoint, self.identity, runtime)?;
        self.machine
            .resume_with_attachment(replacement, peer)
            .map_err(UsbLinkError::Codec)
    }
}

fn binding(
    endpoint: RemoteEndpointIdentity,
    identity: SignalExecutionIdentity,
    runtime: &RuntimeTranscriptIdentity,
) -> Result<SessionBinding, UsbLinkError> {
    let base = ConnectionBase::from_canonical_code(endpoint.base_code)
        .ok_or(UsbLinkError::InvalidGeneratedEndpoint)?;
    if endpoint.local_host != identity.host_id
        || endpoint.local_boot != identity.boot_id
        || endpoint.sink_fragment_id != identity.fragment_id
    {
        return Err(UsbLinkError::InvalidGeneratedEndpoint);
    }
    let plan = PlanId::from(identity.plan_id);
    let source_host = HostId::from(endpoint.peer_host);
    let source_boot = BootId::from(endpoint.peer_boot);
    let sink_host = HostId::from(endpoint.local_host);
    let sink_boot = BootId::from(endpoint.local_boot);
    SessionBinding {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        plan_id: plan.clone(),
        source_fragment_id: FragmentId::from(endpoint.source_fragment_id),
        sink_fragment_id: FragmentId::from(endpoint.sink_fragment_id),
        source_active_play_id: bind_active_play(&plan, &source_host, &source_boot, 0).active_play_id,
        sink_active_play_id: bind_active_play(&plan, &sink_host, &sink_boot, 0).active_play_id,
        connection_id: ConnectionId::from(endpoint.connection_id),
        source: SessionEndpointIdentity { host_id: source_host.clone(), boot_id: source_boot.clone() },
        sink: SessionEndpointIdentity { host_id: sink_host.clone(), boot_id: sink_boot.clone() },
        value_kind: KindId::from(endpoint.value_kind),
        limits: SessionLimits {
            maximum_in_flight_items: endpoint.maximum_in_flight_items,
            maximum_payload_bytes: endpoint.maximum_payload_bytes,
            maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
        },
        attachment: RouteAttachment {
            link_binding_id: LinkBindingId::from(endpoint.link_binding_id),
            base,
            base_instance_id: ConnectionBaseInstanceId::from(endpoint.base_instance_id),
            source_host_id: source_host,
            source_boot_id: source_boot,
            source_endpoint_id: LinkEndpointId::from(endpoint.peer_endpoint),
            sink_host_id: sink_host,
            sink_boot_id: sink_boot,
            sink_endpoint_id: LinkEndpointId::from(endpoint.local_endpoint),
            limits: LinkLimits {
                maximum_in_flight_items: endpoint.maximum_in_flight_items,
                maximum_payload_bytes: endpoint.maximum_payload_bytes,
                maximum_buffered_bytes: endpoint.maximum_buffered_bytes,
                maximum_frame_bytes: endpoint.maximum_frame_bytes,
            },
        },
    }
    .with_observed_boots(BootId::from(endpoint.peer_boot), BootId::from(runtime.boot_id()))
    .map_err(UsbLinkError::Codec)
}
