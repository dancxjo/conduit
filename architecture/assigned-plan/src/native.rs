//! Selected cooperative native-operation facts carried by schema 6. These are
//! descriptive bindings; an actual local opaque capability remains mandatory.
use crate::{AssignedIdentity as Id, AssignedPlanRefusal as E};
pub const ASSIGNED_NATIVE_PLAN_SCHEMA: u16 = 6;
pub const ASSIGNED_NATIVE_CALL_BYTES: usize = 326;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedNativeBinding {
    pub capability: Id,
    pub kind_revision: Id,
    pub execution_profile: Id,
    pub maximum_instances: u16,
    pub queue_items: u16,
    pub queue_bytes: u32,
    pub base: Id,
    pub instance: Id,
    pub generation: u64,
    pub base_implementation: Id,
    pub mechanism: Id,
    pub enforcement: u8,
    pub grant: Id,
    pub authority: Id,
    pub call: Id,
    pub subject: Id,
    pub host: Id,
    pub boot: Id,
    pub authority_capability: Id,
    pub resource: u16,
    pub pool: Id,
    pub resource_class: Id,
    pub units: u32,
}
pub fn assigned_native_binding(payload: &[u8]) -> Result<Option<AssignedNativeBinding>, E> {
    match (payload.len(), payload.get(46)) {
        (47, Some(0)) => return Ok(None),
        (ASSIGNED_NATIVE_CALL_BYTES, Some(1)) => {}
        _ => return Err(E::MalformedRecord),
    }
    let mut at = Cursor {
        bytes: payload,
        at: 47,
    };
    let binding = AssignedNativeBinding {
        capability: at.id()?,
        kind_revision: at.id()?,
        execution_profile: at.id()?,
        maximum_instances: at.u16()?,
        queue_items: at.u16()?,
        queue_bytes: at.u32()?,
        base: at.id()?,
        instance: at.id()?,
        generation: at.u64()?,
        base_implementation: at.id()?,
        mechanism: at.id()?,
        enforcement: at.byte()?,
        grant: at.id()?,
        authority: at.id()?,
        call: at.id()?,
        subject: at.id()?,
        host: at.id()?,
        boot: at.id()?,
        authority_capability: at.id()?,
        resource: at.u16()?,
        pool: at.id()?,
        resource_class: at.id()?,
        units: at.u32()?,
    };
    if binding.enforcement != 0 || binding.generation == 0 || binding.units == 0 {
        return Err(E::MalformedRecord);
    }
    Ok(Some(binding))
}
struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], E> {
        let end = self.at.checked_add(N).ok_or(E::MalformedRecord)?;
        let result = self
            .bytes
            .get(self.at..end)
            .ok_or(E::MalformedRecord)?
            .try_into()
            .map_err(|_| E::MalformedRecord)?;
        self.at = end;
        Ok(result)
    }
    fn id(&mut self) -> Result<Id, E> {
        Ok(Id(self.bytes()?))
    }
    fn byte(&mut self) -> Result<u8, E> {
        Ok(self.bytes::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, E> {
        Ok(u16::from_le_bytes(self.bytes()?))
    }
    fn u32(&mut self) -> Result<u32, E> {
        Ok(u32::from_le_bytes(self.bytes()?))
    }
    fn u64(&mut self) -> Result<u64, E> {
        Ok(u64::from_le_bytes(self.bytes()?))
    }
}
