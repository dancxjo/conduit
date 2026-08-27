use conduit_core::{
    ActivePlayId, BaseImplementationId, BaseInstanceId, BootId, ConnectionId, FragmentId, HostId,
    KindId, LineId, LinkBindingId, LinkEndpointId, PlanId,
};

use super::{
    LineAttachment, SessionBinding, SessionEndpointIdentity, SessionFrame, SessionMessage,
};
use crate::WireError;

impl SessionBinding {
    pub fn from_hello_frame(frame: SessionFrame<'_>) -> Result<Self, WireError> {
        let SessionMessage::Hello(hello) = frame.message else {
            return Err(WireError::InvalidSession);
        };
        let identity = frame.identity;
        let binding = Self {
            protocol_version: identity.protocol_version,
            plan_id: PlanId::from(identity.plan_id),
            source_fragment_id: FragmentId::from(identity.source_fragment_id),
            sink_fragment_id: FragmentId::from(identity.sink_fragment_id),
            source_active_play_id: ActivePlayId::from(identity.source_active_play_id),
            sink_active_play_id: ActivePlayId::from(identity.sink_active_play_id),
            connection_id: ConnectionId::from(identity.connection_id),
            source: SessionEndpointIdentity {
                host_id: HostId::from(identity.source_host_id),
                boot_id: BootId::from(identity.source_boot_id),
            },
            sink: SessionEndpointIdentity {
                host_id: HostId::from(identity.sink_host_id),
                boot_id: BootId::from(identity.sink_boot_id),
            },
            value_kind: KindId::from(identity.value_kind),
            limits: identity.limits,
            attachment: LineAttachment {
                line_id: LineId::from(hello.line_id),
                link_binding_id: LinkBindingId::from(hello.link_binding_id),
                base: BaseImplementationId::from(hello.base),
                base_instance_id: BaseInstanceId::from(hello.base_instance_id),
                contract: hello.contract,
                source_host_id: HostId::from(identity.source_host_id),
                source_boot_id: BootId::from(identity.source_boot_id),
                source_endpoint_id: LinkEndpointId::from(hello.source_endpoint_id),
                sink_host_id: HostId::from(identity.sink_host_id),
                sink_boot_id: BootId::from(identity.sink_boot_id),
                sink_endpoint_id: LinkEndpointId::from(hello.sink_endpoint_id),
                limits: hello.limits,
            },
        };
        binding.validate()?;
        Ok(binding)
    }
}
