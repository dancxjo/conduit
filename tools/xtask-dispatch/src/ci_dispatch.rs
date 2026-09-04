use std::path::PathBuf;

mod pages_resolver;

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    match arguments.get(1).map(String::as_str) {
        Some("plan") => plan(arguments),
        Some("candidate") => candidate(arguments),
        Some("reconcile") => reconcile(arguments),
        Some("reconcile-product") => reconcile_product(arguments),
        Some("attest-success") => attest(arguments),
        Some("standalone-locks") => standalone_locks(arguments),
        Some("pages-resolver-proof") => pages_resolver::run(arguments),
        Some(command) => Err(format!("unsupported ci command: {command}")),
        None => Err("missing ci command".to_owned()),
    }
}

fn reconcile_product(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
    let product_id = required(&mut values, "product proof id")?;
    let candidate = required(&mut values, "candidate commit")?;
    let integration = required(&mut values, "integration commit")?;
    let mut json_out = None;
    let mut allow_execute = false;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--allow-execute" => allow_execute = true,
            "--json-out" => {
                json_out = Some(PathBuf::from(required(&mut values, "--json-out path")?))
            }
            other => {
                return Err(format!(
                    "unsupported ci reconcile-product argument: {other}"
                ))
            }
        }
    }
    crate::product_reconciliation::run(
        &product_id,
        &candidate,
        &integration,
        json_out.as_deref(),
        allow_execute,
    )
    .map_err(|error| error.to_string())
}

fn standalone_locks(arguments: &[String]) -> Result<(), String> {
    for argument in arguments.iter().skip(2) {
        if argument != "--locked" {
            return Err(format!(
                "unsupported ci standalone-locks argument: {argument}"
            ));
        }
    }
    crate::standalone_locks::run().map_err(|error| error.to_string())
}

fn plan(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
    let base = required(&mut values, "base commit")?;
    let head = required(&mut values, "head commit")?;
    let options = common_options(values)?;
    crate::impact::run(
        &base,
        &head,
        options.json_out.as_deref(),
        options.summary_out.as_deref(),
    )
    .map_err(|error| error.to_string())
}

fn candidate(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
    let head = required(&mut values, "candidate commit")?;
    let options = proof_options(values, false)?;
    crate::proof_graph::candidate(
        &head,
        &options.receipts,
        options.common.json_out.as_deref(),
        options.common.summary_out.as_deref(),
    )
    .map_err(|error| error.to_string())
}

fn reconcile(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
    let base = required(&mut values, "base commit")?;
    let head = required(&mut values, "candidate commit")?;
    let options = proof_options(values, false)?;
    crate::proof_graph::reconcile(
        &base,
        &head,
        &options.receipts,
        options.common.json_out.as_deref(),
        options.common.summary_out.as_deref(),
    )
    .map_err(|error| error.to_string())
}

fn attest(arguments: &[String]) -> Result<(), String> {
    let mut values = arguments.iter().skip(2);
    let head = required(&mut values, "candidate commit")?;
    let proof_id = required(&mut values, "proof id")?;
    let mut evidence = Vec::new();
    let mut out = None;
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--evidence" => evidence.push(required(&mut values, "--evidence value")?),
            "--out" => out = Some(PathBuf::from(required(&mut values, "--out path")?)),
            other => return Err(format!("unsupported ci attest-success argument: {other}")),
        }
    }
    let out = out.ok_or_else(|| "ci attest-success requires --out".to_owned())?;
    crate::proof_graph::attest_success(&head, &proof_id, &evidence, &out)
        .map_err(|error| error.to_string())
}

#[derive(Default)]
struct CommonOptions {
    json_out: Option<PathBuf>,
    summary_out: Option<PathBuf>,
}

struct ProofOptions {
    common: CommonOptions,
    receipts: Vec<PathBuf>,
}

fn common_options<'a>(values: impl Iterator<Item = &'a String>) -> Result<CommonOptions, String> {
    let proof = proof_options(values, true)?;
    Ok(proof.common)
}

fn proof_options<'a>(
    mut values: impl Iterator<Item = &'a String>,
    reject_receipts: bool,
) -> Result<ProofOptions, String> {
    let mut common = CommonOptions::default();
    let mut receipts = Vec::new();
    while let Some(argument) = values.next() {
        match argument.as_str() {
            "--locked" => {}
            "--json-out" => {
                common.json_out = Some(PathBuf::from(required(&mut values, "--json-out path")?))
            }
            "--summary-out" => {
                common.summary_out =
                    Some(PathBuf::from(required(&mut values, "--summary-out path")?))
            }
            "--receipt" if !reject_receipts => {
                receipts.push(PathBuf::from(required(&mut values, "--receipt path")?))
            }
            other => return Err(format!("unsupported ci argument: {other}")),
        }
    }
    Ok(ProofOptions { common, receipts })
}

fn required<'a>(
    values: &mut impl Iterator<Item = &'a String>,
    name: &str,
) -> Result<String, String> {
    values
        .next()
        .cloned()
        .ok_or_else(|| format!("missing {name}"))
}
