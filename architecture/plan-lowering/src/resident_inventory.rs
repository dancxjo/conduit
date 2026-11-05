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
    mut identity: impl FnMut(usize) -> Option<&'static ResidentExpressionIdentity>,
) -> Result<ResidentInventorySelection<N>, E> {
    if N == 0 || N > 32 {
        return Err(E::Selection);
    }
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
        if usize::from(node) >= N || result.selected[usize::from(node)].is_some() {
            return Err(E::Selection);
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
        let (entry, mut pin) = found.ok_or(E::MissingOwner)?;
        pin.node = node;
        result.selected[usize::from(node)] = Some(ResidentNodeSelection { node, entry });
        result.pins[result.count] = pin;
        result.count += 1;
    }
    if result.count == 0 || result.selected[..result.count].iter().any(Option::is_none) {
        return Err(E::Selection);
    }
    Ok(result)
}
