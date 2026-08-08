use crate::{
    cli::ProofsArgs,
    proof::{current_catalog, ProofRecord, ProofRequirement, CURRENT_PROOF_COMMANDS},
};

pub fn run(args: ProofsArgs, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(path) = args.validate_record {
        let record: ProofRecord = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let contract = CURRENT_PROOF_COMMANDS
            .iter()
            .find(|contract| contract.command == record.command)
            .ok_or("proof record names an unregistered command")?;
        record.validate_against(contract)?;
        let exact_requirement = ProofRequirement {
            required: contract.proof_class,
            explicitly_accepted_substitutes: &[],
        };
        let satisfies_claims = record.satisfies(contract, &exact_requirement);
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "record": record,
                    "satisfies_claims": satisfies_claims,
                }))?
            );
        } else {
            println!(
                "valid proof record for {}; satisfies claims: {satisfies_claims}",
                contract.id
            );
        }
        return Ok(());
    }
    if json {
        println!("{}", serde_json::to_string_pretty(&current_catalog())?);
    } else {
        println!("Conduit proof contract schema v1");
        for proof in CURRENT_PROOF_COMMANDS {
            println!(
                "{} [{}]\n  {}\n  claims: {}",
                proof.id,
                proof.proof_class.as_str(),
                proof.command,
                proof.allowed_claims.join("; ")
            );
        }
    }
    Ok(())
}
