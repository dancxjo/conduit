//! Typed fresh fields; these are descriptive inputs, never native possession.
use crate::*;
/// Raw encoding inputs, not proof that a Plan/fragment identity was lawfully
/// sealed. Root must obtain fresh identities through canonical sealing (or a
/// checked streamed seal recipe); this codec never invents a Body Plan/Play.
#[derive(Clone, Copy)]
pub struct FreshAssignedHeader {
    pub plan: AssignedIdentity,
    pub fragment: AssignedIdentity,
    pub host: AssignedIdentity,
    pub boot: AssignedIdentity,
}
#[derive(Clone, Copy)]
pub struct FreshAssignedNative {
    pub node: u16,
    pub call: u16,
    pub contract: AssignedIdentity,
    pub binding: AssignedNativeBinding,
}
/// Root supplies the independently installed Source names and ACTUAL fresh
/// placement identity. This DTO proves neither placement nor authority. The
/// template additionally checks exact Fore endpoint, direction and local port.
#[derive(Clone, Copy)]
pub struct FreshAssignedFore<'a> {
    pub cord: u16,
    pub endpoint: u16,
    pub direction: AssignedForeDirection,
    pub local_node: u16,
    pub local_port: u16,
    pub front_port: &'a str,
    pub placement: &'a str,
    pub gear_port: &'a str,
}
/// Same canonical name hashed by lowering.rs's `front/{direction}/{front}/{placement}/{gear}`.
/// No allocation or alternate identity namespace. Caller establishes the actual
/// placement; generating an identity does not establish a reservation or Fore.
pub fn assigned_local_fore_connection(
    direction: AssignedForeDirection,
    front: &str,
    placement: &str,
    gear: &str,
) -> AssignedIdentity {
    let mut hash = crate::sha256::State::new();
    hash.update(b"front/");
    hash.update(if direction == AssignedForeDirection::Input {
        b"0/"
    } else {
        b"1/"
    });
    hash.update(front.as_bytes());
    hash.update(b"/");
    hash.update(placement.as_bytes());
    hash.update(b"/");
    hash.update(gear.as_bytes());
    let digest = hash.finish();
    AssignedIdentity(digest[..16].try_into().expect("digest width"))
}
pub(super) fn static_binding(mut b: AssignedNativeBinding) -> AssignedNativeBinding {
    let zero = AssignedIdentity([0; 16]);
    b.instance = zero;
    b.generation = 1;
    b.grant = zero;
    b.host = zero;
    b.boot = zero;
    b.pool = zero;
    b
}
pub(super) fn external(record: AssignedRecord<'_>) -> bool {
    record.tag == ASSIGNED_CORD
        && record.payload.len() == 42
        && (record.payload[18] == 1 || record.payload[23] == 1)
}
pub(super) fn matching_fore(bytes: &[u8], cord: &[u8]) -> Result<[u8; 53], AssignedPlanRefusal> {
    let input = cord[18] == 1;
    if input == (cord[23] == 1) {
        return Err(AssignedPlanRefusal::MalformedRecord);
    }
    let endpoint = crate::read_u16(cord, if input { 19 } else { 24 })?;
    let ordinal = crate::read_u16(cord, 0)?;
    let mut found = None;
    for record in AssignedPlanRecords::new(bytes)? {
        let record = record?;
        if record.tag == ASSIGNED_LOCAL_FORE
            && record.payload.len() == 53
            && crate::read_u16(record.payload, 0)? == endpoint
            && crate::read_u16(record.payload, 2)? == ordinal
            && record.payload[4] == u8::from(!input)
        {
            if found.is_some() {
                return Err(AssignedPlanRefusal::MalformedRecord);
            }
            found = Some(
                record
                    .payload
                    .try_into()
                    .map_err(|_| AssignedPlanRefusal::MalformedRecord)?,
            );
        }
    }
    found.ok_or(AssignedPlanRefusal::MalformedRecord)
}
