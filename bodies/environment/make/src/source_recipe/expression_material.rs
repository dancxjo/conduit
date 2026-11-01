//! Emit one deduplicated complete Make preparation pool, bound to checked Source.
use std::{
    collections::BTreeMap,
    fmt::{self, Write},
};
pub(super) fn write(
    out: &mut impl Write,
    source: conduit_environment_runtime::appliance::assigned::ResidentSourceIdentity,
    requirements: &[conduit_core::AssignedComputationRequirement],
    texts: &[(u16, conduit_core::AssignedIdentity, String, Vec<u8>)],
) -> fmt::Result {
    use conduit_core as c;
    let key = c::AssignedIdentity::from_text("program");
    let mut blob = Vec::new();
    let mut unique = BTreeMap::<Vec<u8>, (usize, usize)>::new();
    let mut entries = Vec::new();
    for (node, actual_key, _, canonical) in texts {
        if *actual_key != key {
            continue;
        }
        let expected = requirements
            .iter()
            .find(|r| r.node == *node && r.key == *actual_key)
            .ok_or(fmt::Error)?;
        let program = conduit_plot::PortableExpressionProgram::from_canonical_bytes(canonical)
            .map_err(|_| fmt::Error)?;
        let material =
            conduit_plot::MakeExpressionMaterial::export(&program).map_err(|_| fmt::Error)?;
        let bytes = material.bytes().to_vec();
        let range = *unique.entry(bytes.clone()).or_insert_with(|| {
            let r = (blob.len(), bytes.len());
            blob.extend_from_slice(&bytes);
            r
        });
        entries.push((
            *node,
            *actual_key,
            expected.text_digest,
            c::assigned_plan_payload_digest(canonical),
            c::assigned_plan_payload_digest(&bytes),
            program
                .maximum_prepared_input_bytes()
                .map_err(|_| fmt::Error)?,
            program
                .maximum_prepared_output_bytes()
                .map_err(|_| fmt::Error)?,
            range,
        ));
    }
    writeln!(out, "static EXPRESSION_MATERIAL_BYTES: &[u8] = &{blob:?};")?;
    writeln!(out,"static EXPRESSION_MATERIALS: [conduit_environment_runtime::appliance::assigned::ResidentExpressionMaterial; {}] = [",entries.len())?;
    for (
        node,
        key,
        text_digest,
        program_digest,
        material_digest,
        maximum_input_bytes,
        maximum_output_bytes,
        (at, len),
    ) in entries
    {
        writeln!(out,"conduit_environment_runtime::appliance::assigned::ResidentExpressionMaterial {{ source: ResidentSourceIdentity {{expanded_plot_digest: {:?}}},node:{node},key:c::AssignedIdentity({:?}),text_digest:{text_digest:?},program_digest:{program_digest:?},material_digest:{material_digest:?},maximum_input_bytes:{maximum_input_bytes},maximum_output_bytes:{maximum_output_bytes},bytes:EXPRESSION_MATERIAL_BYTES.split_at({at}).1.split_at({len}).0 }},",source.expanded_plot_digest,key.0)?;
    }
    writeln!(out,"];\npub fn expression_materials() -> &'static [conduit_environment_runtime::appliance::assigned::ResidentExpressionMaterial] {{ &EXPRESSION_MATERIALS }}")
}
