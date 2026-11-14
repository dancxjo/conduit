//! Select independently initialized resident identities without decoding catalogs.
//! This is preflight only: no reservation, loan, native possession or Play.
use crate::selected_assigned_expression::{
    ResidentExpressionBindingRefusal as E, ResidentExpressionIdentity,
};
use conduit_core as c;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentNodeSelection {
    pub node: u16,
    pub entry: usize,
}
pub struct ResidentInventorySelection<const N: usize> {
    selected: [Option<ResidentNodeSelection>; N],
    pins: [c::AssignedComputationRequirement; N],
    count: usize,
}
impl<const N: usize> ResidentInventorySelection<N> {
    pub fn nodes(&self) -> impl Iterator<Item = ResidentNodeSelection> + '_ {
        self.selected.iter().flatten().copied()
    }
    pub fn computations(&self) -> &[c::AssignedComputationRequirement] {
        &self.pins[..self.count]
    }
}

/// Inspect only immutable identity hints, then pin exact resident program bytes.
/// The caller must still run Core decoding and every selected Back's binder.
/// `identity` must return only actual initialized, currently available entries.
pub fn select_resident_inventory<const N: usize>(
    projection: &[u8],
    identity: impl FnMut(usize) -> Option<&'static ResidentExpressionIdentity>,
) -> Result<ResidentInventorySelection<N>, E> {
    select_resident_inventory_with_other_nodes(projection, &[], identity)
}

/// Exact non-resident node hints supplied independently by the local owner.
/// These skip computation lookup only; native binding and authority remain
/// mandatory. Incoming assignment identities alone must not supply this list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OtherNodeIdentity {
    pub node: u16,
    pub kind: c::AssignedIdentity,
    pub implementation: c::AssignedIdentity,
    pub artifact: c::AssignedIdentity,
}

/// Select the resident subset of a mixed fragment without decoding an offer
/// catalog. Every other node must match one independently supplied exact hint.
/// This does not validate the projection or authorize any non-resident effect.
pub fn select_resident_inventory_with_other_nodes<const N: usize>(
    projection: &[u8],
    other: &[OtherNodeIdentity],
    mut identity: impl FnMut(usize) -> Option<&'static ResidentExpressionIdentity>,
) -> Result<ResidentInventorySelection<N>, E> {
    if N == 0 || N > 32 || other.len() > 32 - N {
        return Err(E::Selection);
    }
    let maximum_nodes = N + other.len();
    let mut expected_other = 0u32;
    for pin in other {
        if usize::from(pin.node) >= maximum_nodes || expected_other & (1 << pin.node) != 0 {
            return Err(E::Selection);
        }
        expected_other |= 1 << pin.node;
    }
    let mut seen = 0u32;
    let mut seen_other = 0u32;
    let empty = c::AssignedComputationRequirement {
        node: 0,
        key: c::AssignedIdentity([0; 16]),
        text_bytes: 0,
        text_digest: [0; 32],
    };
    let mut result = ResidentInventorySelection {
        selected: [None; N],
        pins: [empty; N],
        count: 0,
    };
    for record in c::AssignedPlanRecords::new(projection).map_err(E::Assignment)? {
        let record = record.map_err(E::Assignment)?;
        if record.tag != c::ASSIGNED_NODE {
            continue;
        }
        let d = record.payload;
        if d.len() != 52 {
            return Err(E::Selection);
        }
        let node = u16::from_le_bytes(d[..2].try_into().map_err(|_| E::Selection)?);
        if usize::from(node) >= maximum_nodes || seen & (1 << node) != 0 {
            return Err(E::Selection);
        }
        seen |= 1 << node;
        let external = other.iter().find(|pin| pin.node == node);
        if let Some(pin) = external {
            if d[4..20] != pin.kind.0
                || d[20..36] != pin.implementation.0
                || d[36..52] != pin.artifact.0
            {
                return Err(E::Selection);
            }
        }
        let mut found = None;
        for entry in 0..N {
            let Some(pin) = identity(entry) else {
                continue;
            };
            if d[4..20] == pin.kind.0
                && d[20..36] == pin.implementation.0
                && d[36..52] == pin.artifact.0
            {
                if found.is_some() || result.nodes().any(|s| s.entry == entry) {
                    return Err(E::Selection);
                }
                found = Some((entry, pin.computation));
            }
        }
        if external.is_some() {
            if found.is_some() {
                return Err(E::Selection);
            }
            seen_other |= 1 << node;
            continue;
        }
        let (entry, mut pin) = found.ok_or(E::MissingOwner)?;
        if result.count == N {
            return Err(E::Selection);
        }
        pin.node = node;
        result.selected[result.count] = Some(ResidentNodeSelection { node, entry });
        result.pins[result.count] = pin;
        result.count += 1;
    }
    let count = result.count + other.len();
    let dense = if count == 32 {
        u32::MAX
    } else {
        (1u32 << count) - 1
    };
    if count == 0 || seen != dense || seen_other != expected_other {
        return Err(E::Selection);
    }
    result.selected[..result.count].sort_unstable_by_key(|pin| pin.unwrap().node);
    Ok(result)
}
