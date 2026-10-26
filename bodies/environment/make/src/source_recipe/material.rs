//! Emit only independently checked portable construction data.
use std::fmt::{self, Write};

pub(crate) fn write_material(
    out: &mut impl Write,
    computations: &[conduit_core::AssignedComputationRequirement],
    texts: &[(u16, conduit_core::AssignedIdentity, String, Vec<u8>)],
    offers: &[(u16, Vec<u8>)],
) -> fmt::Result {
    writeln!(
        out,
        "static COMPUTATIONS: [c::AssignedComputationRequirement; {}] = [",
        computations.len()
    )?;
    for computation in computations {
        writeln!(out, "c::AssignedComputationRequirement {{ node: {}, key: c::AssignedIdentity({:?}), text_bytes: {}, text_digest: {:?} }},", computation.node, computation.key.0, computation.text_bytes, computation.text_digest)?;
    }
    writeln!(
        out,
        "];\nstatic TEXTS: [ResidentComputation<'static>; {}] = [",
        texts.len()
    )?;
    for (node, key, text, bytes) in texts {
        writeln!(out, "ResidentComputation {{ node: {node}, key: c::AssignedIdentity({:?}), text: {text:?}, canonical_bytes: Some(&{bytes:?}) }},", key.0)?;
    }
    writeln!(
        out,
        "];\npub fn computations() -> &'static [ResidentComputation<'static>] {{ &TEXTS }}"
    )?;
    for (index, (_, json)) in offers.iter().enumerate() {
        let offer = serde_json::from_slice(json).map_err(|_| fmt::Error)?;
        super::linked_text::write_offer(out, &format!("decode_offer_{index}"), &offer)?;
    }
    writeln!(
        out,
        "static OFFERS: [ResidentSoftwareOffer<'static>; {}] = [",
        offers.len()
    )?;
    for (index, (node, json)) in offers.iter().enumerate() {
        writeln!(
            out,
            "unsafe {{ ResidentSoftwareOffer::from_linked_make({node}, &{json:?}, decode_offer_{index}) }},"
        )?;
    }
    writeln!(
        out,
        "];\npub fn offers() -> &'static [ResidentSoftwareOffer<'static>] {{ &OFFERS }}"
    )?;
    Ok(())
}
