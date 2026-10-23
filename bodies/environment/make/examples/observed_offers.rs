//! Internal stage of `cargo xtask prove environment software-ready-esp32`.
//! Consume that invocation's local UART capture, never synthesize native facts.
use conduit_environment_make::{
    compile_appliance_source, NativeApplianceLimits, NativeSoftwareSourceRecipe,
    PreparedNativeAppliance,
};
use std::{error::Error, fs, io::Read, path::PathBuf};
#[path = "observed_offers/body.rs"]
mod body;

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(args.next().ok_or("expected capture directory")?);
    let admission = args.next().map(PathBuf::from);
    if args.next().is_some() {
        return Err("expected capture directory and optional local admission directory".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(directory.join("replies.json"))?
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 128 * 1024 {
        return Err("capture exceeded finite host ingress bound".into());
    }
    let replies: Vec<serde_json::Value> = serde_json::from_slice(&bytes)?;
    let [before, software, policy, after] = replies.as_slice() else {
        return Err("expected four scoped Root replies".into());
    };
    for root in [before, after] {
        if root["event"] != "native-root-offer"
            || root["body_installed"] != false
            || root["native_plan_installed"] != false
        {
            return Err("unexpected Root preparation state".into());
        }
    }
    let source = compile_appliance_source().map_err(|why| format!("Source: {why:?}"))?;
    let source_recipe = NativeSoftwareSourceRecipe::prepare(&source)
        .map_err(|why| format!("Source recipe: {why:?}"))?;
    let advertisement = source_recipe
        .observe_native_offers(
            &serde_json::from_value(before["advertisement"].clone())?,
            &serde_json::to_vec(software)?,
            &serde_json::from_value(after["advertisement"].clone())?,
        )
        .map_err(|why| format!("Root offer composition: {why:?}"))?;
    let grants = source_recipe
        .observe_appliance_policy(&advertisement, &serde_json::to_vec(policy)?)
        .map_err(|why| format!("Root policy: {why:?}"))?;
    let mut source_material = String::new();
    source_recipe.write_source_rust(&mut source_material)?;
    let source_records: Vec<_> = source_recipe.resident_computation_records().collect();
    fs::write(directory.join("uninstalled-source.rs"), source_material)?;
    let prepared = PreparedNativeAppliance::prepare(
        &source,
        &advertisement,
        &["conduit.base/local@1".into()],
        &grants,
        NativeApplianceLimits::resident_local(),
    )
    .map_err(|why| format!("Native construction: {why:?}"))?;
    if let Some(admission) = admission {
        body::propose(
            &prepared,
            &serde_json::from_value(before["advertisement"].clone())?,
            &admission,
            &directory,
        )?;
    }
    let plan = prepared.plan();
    if !conduit_core::verify_plan(plan) {
        return Err("ordinary Plan verification refused".into());
    }
    fs::write(
        directory.join("uninstalled-plan.json"),
        serde_json::to_vec_pretty(plan)?,
    )?;
    fs::write(
        directory.join("uninstalled-assignment.bin"),
        prepared.assignment(),
    )?;
    let mut resident = String::new();
    let recipe = prepared
        .software_recipe()
        .map_err(|why| format!("Resident recipe: {why:?}"))?;
    recipe.write_resident_rust(&mut resident)?;
    fs::write(directory.join("uninstalled-resident.rs"), resident)?;
    let mut layout = String::new();
    recipe.write_layout_rust(&mut layout)?;
    fs::write(directory.join("uninstalled-layout.rs"), layout)?;
    let calls: Vec<_> = prepared
        .image()
        .host_calls
        .iter()
        .map(|call| conduit_core::AssignedIdentity::from_text(&call.contract_id))
        .collect();
    let resources: Vec<_> = prepared
        .image()
        .resources
        .iter()
        .map(|resource| resource.resource)
        .collect();
    let required = conduit_core::AssignedPlanRequirements {
        host: conduit_core::AssignedIdentity::from_text(advertisement.host_id.as_str()),
        boot: conduit_core::AssignedIdentity::from_text(advertisement.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let view = conduit_core::decode_assigned_plan_with_computations(
        prepared.assignment(),
        NativeApplianceLimits::resident_local().assignment,
        required,
        recipe.resident().computations,
    )
    .map_err(|why| format!("Assignment validation: {why:?}"))?;
    use conduit_environment_runtime::appliance::assignment_transfer as transfer;
    let assigned_records: Vec<_> = conduit_core::AssignedPlanRecords::new(prepared.assignment())
        .map_err(|why| format!("Assignment records: {why:?}"))?
        .collect::<Result<_, _>>()
        .map_err(|why| format!("Assignment record: {why:?}"))?;
    let mut wire = [0; transfer::MAXIMUM_WIRE_BYTES];
    let wire_size = transfer::encode(
        &prepared.assignment()[..conduit_core::ASSIGNED_PLAN_HEADER_BYTES],
        &assigned_records,
        source_recipe.spec().source,
        &source_records,
        &mut wire,
    )
    .map_err(|why| format!("Assignment transfer encoding: {why:?}"))?;
    fs::write(
        directory.join("uninstalled-assignment-transfer.bin"),
        &wire[..wire_size],
    )?;
    let mut slots = [conduit_core::AssignedRecord {
        tag: 0,
        payload: &[],
    }; transfer::MAXIMUM_RECORDS];
    let decoded = transfer::decode(
        &wire[..wire_size],
        source_recipe.spec().source,
        &source_records,
        &mut slots,
    )
    .map_err(|why| format!("Assignment transfer framing: {why:?}"))?;
    let borrowed_records = decoded.records;
    let borrowed = conduit_core::decode_assigned_records_with_computations(
        decoded.header,
        borrowed_records,
        NativeApplianceLimits::resident_local().assignment,
        required,
        recipe.resident().computations,
    )
    .map_err(|why| format!("Borrowed records: {why:?}"))?;
    if view != borrowed {
        return Err("borrowed and contiguous assigned views differ".into());
    }
    // Compiler round trip on the planning host, not device trust/provisioning.
    conduit_environment_runtime::appliance::assigned::ValidatedAssignedAppliance::admit_records(
        decoded.header,
        borrowed_records,
        NativeApplianceLimits::resident_local().assignment,
        required,
        &recipe.resident(),
    )
    .map_err(|why| format!("Borrowed compiler table round trip: {why:?}"))?;
    let mut record_bytes = [0usize; conduit_core::ASSIGNED_PLAN_COUNT_KINDS];
    let mut computation_bytes = 0usize;
    let mut matched_source_records = 0usize;
    for record in conduit_core::AssignedPlanRecords::new(prepared.assignment())
        .map_err(|why| format!("Assignment records: {why:?}"))?
    {
        let record = record.map_err(|why| format!("Assignment record: {why:?}"))?;
        record_bytes[usize::from(record.tag - 1)] += 3 + record.payload.len();
        if record.tag == conduit_core::ASSIGNED_CONFIGURATION
            && record.payload.len() >= 19
            && record.payload[18] == 3
            && prepared.computations().iter().any(|expected| {
                record.payload[..2] == expected.node.to_le_bytes()
                    && record.payload[2..18] == expected.key.0
            })
        {
            if source_records
                .iter()
                .filter(|expected| **expected == record)
                .count()
                != 1
            {
                return Err("Source-only computation record differs from native projection".into());
            }
            matched_source_records += 1;
            computation_bytes += 3 + record.payload.len();
        }
    }
    if matched_source_records != source_records.len() {
        return Err("native projection omitted a Source computation record".into());
    }
    let receipt = serde_json::json!({
        "proof_class": "source-checked-appliance-planning-from-captured-root-policy",
        "host_id": advertisement.host_id,
        "boot_id": advertisement.boot_id,
        "offer_generation": advertisement.offer_generation,
        "capabilities": advertisement.capabilities.len(),
        "bases": advertisement.bases.len(),
        "resources": advertisement.resources.len(),
        "authority_grants_observed": grants.len(),
        "plan_prepared": true,
        "plan_id": plan.plan_id,
        "fragment_count": plan.fragments.len(),
        "placements": plan.fragments[0].placements.len(),
        "cords": plan.fragments[0].connections.len(),
        "assignment_bytes": prepared.assignment().len(),
        "assignment_transfer_bytes": wire_size,
        "assignment_transfer_delivered_to_device": false,
        "borrowed_record_envelope_validated": true,
        "borrowed_table_compiler_round_trip": true,
        "assignment_record_bytes": record_bytes,
        "resident_computation_record_bytes": computation_bytes,
        "matched_source_computation_records": matched_source_records,
        "assignment_without_resident_records_bytes": prepared.assignment().len() - computation_bytes,
        "projected_runtime_state_bytes": view.runtime_state_bytes,
        "capability_possession_issued": false,
        "body_started": false,
        "plan_installed": false
    });
    fs::write(
        directory.join("planning-advertisement.json"),
        serde_json::to_vec_pretty(&advertisement)?,
    )?;
    fs::write(
        directory.join("composition.json"),
        serde_json::to_vec_pretty(&receipt)?,
    )?;
    println!("{receipt}");
    Ok(())
}
