//! Versioned birth identity, including attributable birth evidence for new Bodies.
use crate::{identity::bind_identity, BodyId, BodyWorkset};
use alloc::vec::Vec;
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BodyIdentityDerivation {
    /// Retained legacy identity: canonical initial workset and birth sequence.
    /// Keep this first variant so existing binary encodings remain readable.
    InitialWorksetV2,
    /// Initial workset, sequence, and exact birth Sign. A Host/Boot-bound Sign
    /// distinguishes independent births of identical software.
    InitialWorksetBirthSignV3,
}

pub(super) fn bind_body(
    initial_workset: &BodyWorkset,
    birth_sequence: u64,
    birth_sign: &SignId,
    derivation: BodyIdentityDerivation,
) -> BodyId {
    let mut values = Vec::with_capacity(2 + initial_workset.len() * 2);
    match derivation {
        BodyIdentityDerivation::InitialWorksetV2 => values.push("conduit.body/identity@2"),
        BodyIdentityDerivation::InitialWorksetBirthSignV3 => {
            values.push("conduit.body/identity@3");
            values.push(birth_sign.as_str());
        }
    }
    for plot in initial_workset.plots() {
        values.push(plot.source_document_id.as_str());
        values.push(plot.checked_plot_id.as_str());
    }
    BodyId::bound(bind_identity("body", &values, birth_sequence))
}
