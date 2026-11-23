//! Shared live model selection; defaults and environment admission stay unchanged.
pub(super) fn ollama_episode_model(
) -> Box<dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter> {
    use conduit_ai::LocalModelKindProfile;
    use conduit_std_host::hosted_local_model::OllamaDiscovery;

    let model = std::env::var("CONDUIT_PETE_LIVE_MODEL").unwrap_or_else(|_| "gemma3:latest".into());
    let memory_mib = std::env::var("CONDUIT_PETE_LIVE_MEMORY_MIB")
        .unwrap_or_else(|_| "6144".into())
        .parse()
        .expect("CONDUIT_PETE_LIVE_MEMORY_MIB must be an integer");
    println!("Live Pete model: {model}; admitted memory: {memory_mib} MiB");
    let adapter = OllamaDiscovery::discover_at("http://127.0.0.1:11434", &model)
        .unwrap()
        .initialize(memory_mib, vec![LocalModelKindProfile::GenerateFlow])
        .unwrap();
    Box::new(adapter)
}

pub(super) fn receipt(
    model: &dyn conduit_std_host::hosted_local_model::HostedLocalModelAdapter,
    source: &str,
    instruction_profile: &str,
) {
    use sha2::{Digest, Sha256};
    if let Ok(expected) = std::env::var("CONDUIT_PETE_LIVE_MODEL_DIGEST") {
        assert_eq!(model.offer().identity.model_content_identity, expected);
    }
    println!(
        "CONDUIT_MODEL_QUALIFICATION_EVIDENCE={}",
        serde_json::json!({
            "proof_class": "live_local_model_with_synthetic_inputs",
            "endpoint": "http://127.0.0.1:11434",
            "instruction_profile": instruction_profile,
            "plot_source_sha256": format!("{:x}", Sha256::digest(source.as_bytes())),
            "plot_source": source,
            "initialized_offer": model.offer(),
        })
    );
}
