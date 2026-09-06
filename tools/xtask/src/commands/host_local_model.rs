use crate::cli::GlobalOpts;
use conduit_ai::LocalModelKindProfile;
use conduit_std_host::hosted_local_model::{HostedLocalModelAdapter, OllamaDiscovery};

pub(super) fn inspect(model: &str, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!("would inspect already-local model {model} without loading it");
        }
        return Ok(());
    }
    let discovery = OllamaDiscovery::discover(model)?;
    if opts.json {
        println!("{}", serde_json::to_string(&discovery)?);
    } else if !opts.quiet {
        println!("LOCAL MODEL DISCOVERED (not initialized or advertised)");
        println!("runtime: {}", discovery.runtime_version);
        println!(
            "model: {} {} ({} bytes)",
            discovery.model_name, discovery.model_content_identity, discovery.model_bytes
        );
        println!(
            "profile: {} {} {} context={}",
            discovery.architecture,
            discovery.parameter_profile,
            discovery.quantization,
            discovery.context_length
        );
    }
    Ok(())
}

pub(super) fn prove(
    model: &str,
    admitted_memory_mib: u32,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.dry_run {
        if !opts.quiet {
            println!(
                "would initialize already-local model {model} with {admitted_memory_mib} MiB admitted"
            );
        }
        return Ok(());
    }
    let adapter = OllamaDiscovery::discover(model)?.initialize(
        admitted_memory_mib,
        vec![
            LocalModelKindProfile::Generate,
            LocalModelKindProfile::ClassifyFiniteLabels,
            LocalModelKindProfile::ExtractValidatedInfo,
            LocalModelKindProfile::InterpretSignEvidence,
        ],
    )?;
    let offer = adapter.offer().clone();
    let receipt = conduit_std_host::local_model_proof::run(adapter)?;
    if opts.json {
        println!("{}", serde_json::to_string(&receipt)?);
    } else if !opts.quiet {
        println!("LOCAL MODEL INITIALIZED AND WARM");
        println!(
            "implementation: {}/{}",
            conduit_ai::LOCAL_MODEL_IMPLEMENTATION,
            offer.identity.model_content_identity
        );
        println!(
            "limits: input={} output={} work={} memory={}MiB in-flight={} queue={}/{}B cancellation={}",
            offer.limits.work.maximum_input_bytes,
            offer.limits.work.maximum_output_bytes,
            offer.limits.work.maximum_work_units,
            offer.limits.admitted_memory_mib,
            offer.limits.maximum_in_flight,
            offer.limits.maximum_queue_items,
            offer.limits.maximum_queue_bytes,
            offer.limits.cancellation_supported
        );
        println!(
            "Plans: generate={} classify={} extract={} interpret={} completed={}/{}/{}/{}",
            receipt.generate_plan_id,
            receipt.classify_plan_id,
            receipt.extract_plan_id,
            receipt.interpret_plan_id,
            receipt.generate_play_completed,
            receipt.classify_play_completed,
            receipt.extract_play_completed,
            receipt.interpret_play_completed
        );
    }
    Ok(())
}
