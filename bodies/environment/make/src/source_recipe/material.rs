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
    write_selector_schemas(out, computations, texts)?;
    Ok(())
}

/// Canonical Types come only from the same ordinarily decoded checked selector
/// configuration. Deduplicate by complete bytes, never by a hash alone.
fn write_selector_schemas(
    out: &mut impl Write,
    computations: &[conduit_core::AssignedComputationRequirement],
    texts: &[(u16, conduit_core::AssignedIdentity, String, Vec<u8>)],
) -> fmt::Result {
    use std::collections::BTreeMap;
    let key = conduit_core::AssignedIdentity::from_text("selector");
    let mut blob = Vec::new();
    let mut types = BTreeMap::<Vec<u8>, (usize, usize)>::new();
    let mut descriptors = Vec::new();
    for (node, actual_key, _, bytes) in texts {
        if *actual_key != key {
            continue;
        }
        let selector = conduit_core::StructuredSelector::from_canonical_bytes(bytes)
            .map_err(|_| fmt::Error)?;
        let expected = computations
            .iter()
            .find(|c| c.node == *node && c.key == *actual_key)
            .ok_or(fmt::Error)?;
        let mut ranges = Vec::new();
        for value_type in [selector.input_type(), selector.output_type()] {
            let canonical = value_type.canonical_bytes().map_err(|_| fmt::Error)?;
            let range = *types.entry(canonical.clone()).or_insert_with(|| {
                let range = (blob.len(), canonical.len());
                blob.extend_from_slice(&canonical);
                range
            });
            ranges.push(range);
        }
        descriptors.push((
            *node,
            *actual_key,
            expected.text_digest,
            ranges[0],
            ranges[1],
        ));
    }
    writeln!(out, "static SELECTOR_SCHEMA_BYTES: &[u8] = &{blob:?};")?;
    writeln!(out, "static SELECTOR_SCHEMAS: [conduit_environment_runtime::appliance::assigned::ResidentSelectorSchemas; {}] = [", descriptors.len())?;
    for (node, key, digest, (input_at, input_len), (output_at, output_len)) in descriptors {
        writeln!(out, "conduit_environment_runtime::appliance::assigned::ResidentSelectorSchemas {{ node: {node}, key: c::AssignedIdentity({:?}), text_digest: {digest:?}, input_schema: SELECTOR_SCHEMA_BYTES.split_at({input_at}).1.split_at({input_len}).0, output_schema: SELECTOR_SCHEMA_BYTES.split_at({output_at}).1.split_at({output_len}).0 }},", key.0)?;
    }
    writeln!(out, "];\npub fn selector_schemas() -> &'static [conduit_environment_runtime::appliance::assigned::ResidentSelectorSchemas] {{ &SELECTOR_SCHEMAS }}")
}
