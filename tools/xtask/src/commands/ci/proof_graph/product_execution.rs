use std::collections::BTreeSet;

use serde::Serialize;

use super::spec::{ProofKind, PROOFS};

const SCHEMA: &str = "conduit.ci.product-execution/v1";
const MAXIMUM_PROOF_IDS_JSON_BYTES: usize = 8 * 1024;

#[derive(Debug, Serialize)]
struct ProductExecution {
    schema: &'static str,
    proof_ids: Vec<String>,
    required: bool,
    browser_runtime_required: bool,
    tour_required: bool,
    patchbay_debugger_required: bool,
    pages_carrier_required: bool,
}

pub(super) fn emit(proof_ids_json: &str) -> Result<(), Box<dyn std::error::Error>> {
    if proof_ids_json.len() > MAXIMUM_PROOF_IDS_JSON_BYTES {
        return Err("execute product proof ID JSON exceeds the finite input bound".into());
    }
    let proof_ids: Vec<String> = serde_json::from_str(proof_ids_json).map_err(|error| {
        format!("execute product proof IDs must be a JSON string array: {error}")
    })?;
    let plan = build_plan(proof_ids)?;

    println!("exact_execution=true");
    println!("required={}", plan.required);
    println!("proofs={}", serde_json::to_string(&plan.proof_ids)?);
    println!("browser_runtime_required={}", plan.browser_runtime_required);
    println!("tour_required={}", plan.tour_required);
    println!(
        "patchbay_debugger_required={}",
        plan.patchbay_debugger_required
    );
    println!("pages_carrier_required={}", plan.pages_carrier_required);
    println!("execution_plan={}", serde_json::to_string(&plan)?);
    Ok(())
}

fn build_plan(proof_ids: Vec<String>) -> Result<ProductExecution, Box<dyn std::error::Error>> {
    if proof_ids.len() > PROOFS.len() {
        return Err("execute product proof ID count exceeds the finite registry".into());
    }
    let unique: BTreeSet<_> = proof_ids.iter().collect();
    if unique.len() != proof_ids.len() {
        return Err("execute product proof IDs must be unique".into());
    }

    let mut plan = ProductExecution {
        schema: SCHEMA,
        required: !proof_ids.is_empty(),
        proof_ids: proof_ids.clone(),
        browser_runtime_required: false,
        tour_required: false,
        patchbay_debugger_required: false,
        pages_carrier_required: false,
    };
    for proof_id in &proof_ids {
        let spec = PROOFS
            .iter()
            .find(|spec| spec.id == proof_id)
            .ok_or_else(|| format!("unknown execute product proof ID {proof_id}"))?;
        if spec.kind != ProofKind::Browser {
            return Err(format!("proof {proof_id} is not a product-lane proposition").into());
        }
        match proof_id.as_str() {
            "browser.tour" => {
                plan.browser_runtime_required = true;
                plan.tour_required = true;
            }
            "browser.patchbay-debugger" => {
                plan.browser_runtime_required = true;
                plan.patchbay_debugger_required = true;
            }
            "products.pages-carrier" => {
                plan.browser_runtime_required = true;
                plan.pages_carrier_required = true;
            }
            _ => {
                return Err(
                    format!("product proof {proof_id} has no exact workflow projection").into(),
                );
            }
        }
    }
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tour_delta_does_not_fabricate_the_machine_universe() {
        let plan = build_for_test(&["browser.tour"]).unwrap();
        assert!(plan.required);
        assert!(plan.browser_runtime_required);
        assert!(plan.tour_required);
        assert!(!plan.patchbay_debugger_required);
        assert!(!plan.pages_carrier_required);
    }

    #[test]
    fn carrier_and_debugger_remain_distinct_propositions() {
        let plan =
            build_for_test(&["browser.patchbay-debugger", "products.pages-carrier"]).unwrap();
        assert!(plan.patchbay_debugger_required);
        assert!(plan.pages_carrier_required);
        assert!(!plan.tour_required);
    }

    #[test]
    fn unknown_duplicate_check_and_oversized_inputs_fail_closed() {
        assert!(build_for_test(&["unknown.proof"]).is_err());
        assert!(build_for_test(&["browser.tour", "browser.tour"]).is_err());
        assert!(build_for_test(&["workspace.lint"]).is_err());
        assert!(emit(&format!(
            "[\"{}\"]",
            "x".repeat(MAXIMUM_PROOF_IDS_JSON_BYTES)
        ))
        .is_err());
    }

    fn build_for_test(ids: &[&str]) -> Result<ProductExecution, Box<dyn std::error::Error>> {
        build_plan(ids.iter().map(|id| (*id).to_owned()).collect())
    }
}
