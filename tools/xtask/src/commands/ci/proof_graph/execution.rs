use std::collections::BTreeSet;

use serde::Serialize;

use super::spec::{Selection, PROOFS};

const SCHEMA: &str = "conduit.ci.check-execution/v1";
const MAXIMUM_PROOF_IDS_JSON_BYTES: usize = 8 * 1024;

#[derive(Debug, Serialize)]
struct CheckExecution {
    schema: &'static str,
    proof_ids: Vec<String>,
    ci_controller_required: bool,
    workspace_matrix: Vec<&'static str>,
    standalone_locks_required: bool,
    esp32_required: bool,
    esp32_matrix: Vec<&'static str>,
    conduitos_required: bool,
    conduitos_limine_required: bool,
    conduitos_tools_required: bool,
    conduitos_x86_matrix: Vec<&'static str>,
    conduitos_architecture_matrix: Vec<&'static str>,
    conduitos_aarch64_product_required: bool,
}

pub(super) fn emit(proof_ids_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    if proof_ids_json.len() > MAXIMUM_PROOF_IDS_JSON_BYTES {
        return Err("execute proof ID JSON exceeds the finite input bound".into());
    }
    let proof_ids: Vec<String> = serde_json::from_str(proof_ids_json)
        .map_err(|error| format!("execute proof IDs must be a JSON string array: {error}"))?;
    let plan = build_plan(proof_ids)?;

    println!("exact_execution=true");
    println!("impact_reason=exact-proof-reconciliation");
    println!("ci_controller_required={}", plan.ci_controller_required);
    println!(
        "workspace_matrix={}",
        serde_json::to_string(&plan.workspace_matrix)?
    );
    println!(
        "standalone_locks_required={}",
        plan.standalone_locks_required
    );
    println!("esp32_required={}", plan.esp32_required);
    println!(
        "esp32_matrix={}",
        serde_json::to_string(&plan.esp32_matrix)?
    );
    println!("conduitos_required={}", plan.conduitos_required);
    println!(
        "conduitos_limine_required={}",
        plan.conduitos_limine_required
    );
    println!("conduitos_tools_required={}", plan.conduitos_tools_required);
    println!(
        "conduitos_x86_matrix={}",
        serde_json::to_string(&plan.conduitos_x86_matrix)?
    );
    println!(
        "conduitos_architecture_matrix={}",
        serde_json::to_string(&plan.conduitos_architecture_matrix)?
    );
    println!(
        "conduitos_aarch64_product_required={}",
        plan.conduitos_aarch64_product_required
    );
    println!("execution_plan={}", serde_json::to_string(&plan)?);
    Ok(())
}

fn build_plan(proof_ids: Vec<String>) -> Result<CheckExecution, Box<dyn std::error::Error>> {
    if proof_ids.len() > PROOFS.len() {
        return Err("execute proof ID count exceeds the finite registry".into());
    }
    let unique: BTreeSet<_> = proof_ids.iter().collect();
    if unique.len() != proof_ids.len() {
        return Err("execute proof IDs must be unique".into());
    }

    let mut plan = CheckExecution {
        schema: SCHEMA,
        proof_ids: proof_ids.clone(),
        ci_controller_required: false,
        workspace_matrix: Vec::new(),
        standalone_locks_required: false,
        esp32_required: false,
        esp32_matrix: Vec::new(),
        conduitos_required: false,
        conduitos_limine_required: false,
        conduitos_tools_required: false,
        conduitos_x86_matrix: Vec::new(),
        conduitos_architecture_matrix: Vec::new(),
        conduitos_aarch64_product_required: false,
    };

    for proof_id in &proof_ids {
        let spec = PROOFS
            .iter()
            .find(|spec| spec.id == proof_id)
            .ok_or_else(|| format!("unknown execute proof ID {proof_id}"))?;
        match spec.selection {
            Selection::CiController => plan.ci_controller_required = true,
            // Reconciliation executes this prerequisite in its dedicated job
            // before invoking the reusable check workflow.
            Selection::SharedCompile => {}
            Selection::WorkspaceShard(shard) => plan.workspace_matrix.push(shard),
            Selection::Esp32Required => plan.standalone_locks_required = true,
            Selection::Esp32Target(target) => {
                plan.esp32_required = true;
                plan.esp32_matrix.push(target);
            }
            Selection::ConduitosRequired if proof_id == "conduitos.limine" => {
                plan.conduitos_required = true;
                plan.conduitos_limine_required = true;
            }
            Selection::ConduitosRequired if proof_id == "conduitos.tools" => {
                plan.conduitos_required = true;
                plan.conduitos_tools_required = true;
            }
            Selection::ConduitosX86(proof) => {
                plan.conduitos_required = true;
                plan.conduitos_limine_required = true;
                plan.conduitos_tools_required = true;
                plan.conduitos_x86_matrix.push(proof);
            }
            Selection::ConduitosArchitecture(architecture) => {
                plan.conduitos_required = true;
                plan.conduitos_limine_required = true;
                plan.conduitos_tools_required = true;
                plan.conduitos_architecture_matrix.push(architecture);
            }
            Selection::ConduitosAarch64Product => {
                plan.conduitos_required = true;
                plan.conduitos_limine_required = true;
                plan.conduitos_tools_required = true;
                plan.conduitos_aarch64_product_required = true;
            }
            Selection::PagesProducts
            | Selection::PagesProductProof(_)
            | Selection::ConduitosRequired => {
                return Err(format!(
                    "proof {proof_id} is not an independently schedulable check proposition"
                )
                .into());
            }
        }
    }
    plan.workspace_matrix.sort_unstable();
    plan.esp32_matrix.sort_unstable();
    plan.conduitos_x86_matrix.sort_unstable();
    plan.conduitos_architecture_matrix.sort_unstable();

    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controller_delta_selects_no_expensive_proof_world() {
        let plan = build_for_test(&["ci.controller-contracts"]).unwrap();
        assert!(plan.ci_controller_required);
        assert!(plan.workspace_matrix.is_empty());
        assert!(!plan.esp32_required);
        assert!(!plan.conduitos_required);
    }

    #[test]
    fn exact_machine_delta_retains_only_named_propositions() {
        let plan = build_for_test(&[
            "machine.esp32-c3",
            "conduitos.x86.usb",
            "conduitos.architecture.ia32",
        ])
        .unwrap();
        assert_eq!(plan.esp32_matrix, ["c3"]);
        assert_eq!(plan.conduitos_x86_matrix, ["usb"]);
        assert_eq!(plan.conduitos_architecture_matrix, ["ia32"]);
        assert!(!plan.standalone_locks_required);
        assert!(plan.conduitos_limine_required);
        assert!(plan.conduitos_tools_required);
    }

    #[test]
    fn unknown_duplicate_and_product_lane_ids_fail_closed() {
        assert!(validate_for_test(&["unknown.proof"]).is_err());
        assert!(validate_for_test(&["workspace.lint", "workspace.lint"]).is_err());
        assert!(validate_for_test(&["browser.tour"]).is_err());
        assert!(emit(&format!(
            "[\"{}\"]",
            "x".repeat(MAXIMUM_PROOF_IDS_JSON_BYTES)
        ))
        .is_err());
    }

    fn validate_for_test(ids: &[&str]) -> Result<CheckExecution, Box<dyn std::error::Error>> {
        build(ids.iter().map(|id| (*id).to_owned()).collect())
    }

    fn build_for_test(ids: &[&str]) -> Result<CheckExecution, Box<dyn std::error::Error>> {
        validate_for_test(ids)
    }

    fn build(proof_ids: Vec<String>) -> Result<CheckExecution, Box<dyn std::error::Error>> {
        build_plan(proof_ids)
    }
}
