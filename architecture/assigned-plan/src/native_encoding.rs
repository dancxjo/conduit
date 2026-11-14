//! Allocation-free inverse of the native-binding payload decoder. Encoding
//! descriptive facts neither binds a provider nor issues execution authority.
use super::{
    AssignedNativeBinding, ASSIGNED_NATIVE_CALL_BYTES, ASSIGNED_NATIVE_CALL_PREFIX_BYTES, E,
};

/// Preserve the common Host Call prefix and encode the existing schema6 native
/// presence/binding suffix. Returns47 bytes without a binding or326 with one.
/// The caller supplies actual scope; this function does not authorize rebinding
/// old approvals. Length/structural refusal leaves the entire output unchanged.
/// Prefix semantics and full assigned-record checks remain decoder obligations.
pub fn encode_assigned_native_binding(
    prefix: &[u8; ASSIGNED_NATIVE_CALL_PREFIX_BYTES],
    binding: Option<&AssignedNativeBinding>,
    output: &mut [u8],
) -> Result<usize, E> {
    let length = if binding.is_some() {
        ASSIGNED_NATIVE_CALL_BYTES
    } else {
        47
    };
    if binding.is_some_and(|b| !b.valid()) {
        return Err(E::MalformedRecord);
    }
    if output.len() < length {
        return Err(E::WrongLength);
    }
    let mut writer = Writer {
        output: &mut output[..length],
        at: 0,
    };
    writer.bytes(prefix);
    writer.bytes(&[u8::from(binding.is_some())]);
    if let Some(b) = binding {
        for id in [b.capability, b.kind_revision, b.execution_profile] {
            writer.bytes(&id.0);
        }
        writer.bytes(&b.maximum_instances.to_le_bytes());
        writer.bytes(&b.queue_items.to_le_bytes());
        writer.bytes(&b.queue_bytes.to_le_bytes());
        writer.bytes(&b.base.0);
        writer.bytes(&b.instance.0);
        writer.bytes(&b.generation.to_le_bytes());
        writer.bytes(&b.base_implementation.0);
        writer.bytes(&b.mechanism.0);
        writer.bytes(&[b.enforcement]);
        for id in [
            b.grant,
            b.authority,
            b.call,
            b.subject,
            b.host,
            b.boot,
            b.authority_capability,
        ] {
            writer.bytes(&id.0);
        }
        writer.bytes(&b.resource.to_le_bytes());
        writer.bytes(&b.pool.0);
        writer.bytes(&b.resource_class.0);
        writer.bytes(&b.units.to_le_bytes());
    }
    debug_assert_eq!(writer.at, length);
    Ok(length)
}
struct Writer<'a> {
    output: &'a mut [u8],
    at: usize,
}
impl Writer<'_> {
    fn bytes(&mut self, bytes: &[u8]) {
        let end = self.at + bytes.len();
        self.output[self.at..end].copy_from_slice(bytes);
        self.at = end;
    }
}
