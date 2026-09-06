use crate::{
    cli::ProofsArgs,
    proof::{current_catalog, ProofRecord, ProofRequirement, CURRENT_PROOF_COMMANDS},
};

pub fn run(args: ProofsArgs, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    if args.run_obligation {
        return run_obligation_command(args, json);
    }
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

fn run_obligation_command(args: ProofsArgs, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()?;
    if !commit.status.success() {
        return Err("cannot resolve exact source commit".into());
    }
    let basis = crate::obligation::ObligationBasis::current(
        String::from_utf8(commit.stdout)?.trim().into(),
    );
    let prior = args
        .resume
        .map(std::fs::read_to_string)
        .transpose()?
        .map(|value| serde_json::from_str(&value))
        .transpose()?;
    let record =
        crate::obligation::run_obligation(basis, prior, args.interrupt_after_checkpoint, || {
            std::process::Command::new(std::env::current_exe().expect("current xtask executable"))
                .args(["--json", "proofs"])
                .output()
                .is_ok_and(|output| {
                    output.status.success()
                        && serde_json::from_slice::<serde_json::Value>(&output.stdout).is_ok_and(
                            |actual| {
                                serde_json::to_value(current_catalog())
                                    .is_ok_and(|expected| actual == expected)
                            },
                        )
                })
        })
        .map_err(|refusal| format!("obligation refused: {refusal:?}"))?;
    let encoded = serde_json::to_string_pretty(&record)?;
    if let Some(path) = args.obligation_record {
        std::fs::write(path, format!("{encoded}\n"))?;
    }
    if json {
        println!("{encoded}");
    } else {
        println!(
            "obligation {} verdict={} attempts={} retention-gap={}",
            record.obligation_id,
            record
                .terminal_verdict
                .as_ref()
                .map_or("interrupted", |verdict| match verdict {
                    crate::obligation::ObligationVerdict::Completed => "completed",
                    crate::obligation::ObligationVerdict::Interrupted => "interrupted",
                    crate::obligation::ObligationVerdict::Failed => "failed",
                }),
            record.attempts.len(),
            record.retention_gap,
        );
    }
    if record.terminal_verdict == Some(crate::obligation::ObligationVerdict::Failed) {
        return Err("proof-catalog validation step failed".into());
    }
    Ok(())
}
