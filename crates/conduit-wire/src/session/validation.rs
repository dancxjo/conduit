use super::{SessionBinding, SessionHello, SessionIdentity};
use crate::WireError;

pub(super) fn validate_identity(
    binding: &SessionBinding,
    identity: SessionIdentity<'_>,
) -> Result<(), WireError> {
    let expected = binding.identity();
    if identity.protocol_version != expected.protocol_version {
        return Err(WireError::WrongProtocolVersion);
    }
    if identity.plan_id != expected.plan_id {
        return Err(WireError::PlanMismatch);
    }
    if identity.source_boot_id != expected.source_boot_id
        || identity.sink_boot_id != expected.sink_boot_id
    {
        return Err(WireError::BootMismatch);
    }
    if identity.connection_id != expected.connection_id {
        return Err(WireError::ConnectionMismatch);
    }
    if identity.value_kind != expected.value_kind {
        return Err(WireError::ValueContractMismatch);
    }
    if identity.limits != expected.limits {
        return Err(WireError::InvalidLimits);
    }
    if identity != expected {
        return Err(WireError::InvalidSession);
    }
    Ok(())
}

pub(super) fn validate_hello(
    binding: &SessionBinding,
    hello: SessionHello<'_>,
) -> Result<(), WireError> {
    if hello.line_id != binding.attachment.line_id.as_str()
        || hello.link_binding_id != binding.attachment.link_binding_id.as_str()
        || hello.base_instance_id != binding.attachment.base_instance_id.as_str()
    {
        return Err(WireError::SessionEpochMismatch);
    }
    if hello.limits != binding.attachment.limits {
        return Err(WireError::InvalidLimits);
    }
    if hello.base != binding.attachment.base
        || hello.source_endpoint_id != binding.attachment.source_endpoint_id.as_str()
        || hello.sink_endpoint_id != binding.attachment.sink_endpoint_id.as_str()
    {
        return Err(WireError::InvalidSession);
    }
    Ok(())
}
