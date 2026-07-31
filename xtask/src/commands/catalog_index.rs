use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use conduit_compile::builtin_catalog_document;
use conduit_runtime::{OwnedNodeSchema, Registry};
use serde::Serialize;

const INVENTORY_PATH: &str = "library/catalog.json";
const INDEX_PATH: &str = "docs/library-tour-index.md";

#[derive(Clone, Copy)]
struct Ownership {
    classification: &'static str,
    package_owner: &'static str,
}

#[derive(Serialize)]
struct Inventory {
    schema: &'static str,
    schema_version: u32,
    source_lowering_rule: SourceLoweringRule,
    entries: Vec<Entry>,
}

#[derive(Serialize)]
struct SourceLoweringRule {
    rule: &'static str,
    aliases_active: bool,
}

#[derive(Serialize)]
struct Entry {
    semantic_identity: String,
    public_source_spelling: String,
    classification: &'static str,
    package_owner: &'static str,
    contract_package_artifact: String,
    export_path: String,
    schema_version: u32,
    semantic_hash: String,
    ports: Vec<Port>,
    config: Vec<ConfigField>,
    catalog_membership: &'static str,
    compiler_exported: bool,
    known_provider_bundles: Vec<Provider>,
    current_provider_observation: &'static str,
    conformance_fixture_owner: &'static str,
    required_result_profile: &'static str,
    structural_facet_owner: String,
    standalone_lesson: Lesson,
    composition_lesson: Lesson,
    successor: Option<String>,
    deprecation: Option<String>,
    compatible_adapter_artifacts: Vec<String>,
}

#[derive(Serialize)]
struct Port {
    id: String,
    direction: &'static str,
    value_type: String,
    type_schema_version: u32,
    type_hash: String,
    presence: &'static str,
    connections: &'static str,
    values: &'static str,
    delivery: &'static str,
    temporal: &'static str,
    terminal: &'static str,
    sensitivity: &'static str,
    loss: &'static str,
}

#[derive(Serialize)]
struct ConfigField {
    key: String,
    value_type: String,
    type_schema_version: u32,
    type_hash: String,
}

#[derive(Serialize)]
struct Provider {
    implementation: String,
    artifact: String,
    artifact_digest: String,
}

#[derive(Serialize)]
struct Lesson {
    artifact: String,
    status: &'static str,
}

fn ownership(id: &str) -> Result<Ownership, String> {
    let value = if id.starts_with("conduit.std/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.flow",
        }
    } else if id.starts_with("std/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std",
        }
    } else if id.starts_with("flow/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.flow",
        }
    } else if id.starts_with("time/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.time",
        }
    } else if id.starts_with("state/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.state",
        }
    } else if id.starts_with("supervision/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.supervision",
        }
    } else if id.starts_with("crypto/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.crypto",
        }
    } else if id.starts_with("data/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.data",
        }
    } else if id.starts_with("display/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.display",
        }
    } else if id.starts_with("text/") {
        Ownership {
            classification: "portable-standard",
            package_owner: "conduit.std.text",
        }
    } else if id.starts_with("io/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.io",
        }
    } else if id.starts_with("fs/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.fs",
        }
    } else if id.starts_with("storage/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.storage",
        }
    } else if id.starts_with("process/") || id.starts_with("conduit.host/process/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.process",
        }
    } else if id.starts_with("device/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.device",
        }
    } else if id.starts_with("secret/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.secret",
        }
    } else if id.starts_with("net/") || id.starts_with("conduit.host/net/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.net",
        }
    } else if id.starts_with("transport/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.transport",
        }
    } else if id.starts_with("ai/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.ai",
        }
    } else if id.starts_with("knowledge/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.knowledge",
        }
    } else if id.starts_with("learned/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.learned",
        }
    } else if id.starts_with("media/") || id.starts_with("conduit.media/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.media",
        }
    } else if id.starts_with("robotics/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.robotics",
        }
    } else if id.starts_with("spatial/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.spatial",
        }
    } else if id.starts_with("speech/") {
        Ownership {
            classification: "reusable-domain-package",
            package_owner: "conduit.domain.speech",
        }
    } else if id.starts_with("evidence/") {
        Ownership {
            classification: "optional-host-boundary",
            package_owner: "conduit.host.evidence",
        }
    } else if id.starts_with("test/") || id.starts_with("observe/") {
        Ownership {
            classification: "implementation-helper",
            package_owner: "conduit.testing",
        }
    } else {
        return Err(format!(
            "catalog entry `{id}` has no explicit package owner/classification"
        ));
    };
    Ok(value)
}

fn fixture(id: &str, classification: &str) -> &'static str {
    if id == "std/text/lines" || id == "std/text/join" {
        "conformance/c4/text-lines-join.json"
    } else if id == "std/text/format" || id == "std/format-values/literal" {
        "conformance/c4/text-format.json"
    } else if id == "supervision/supervisor" || id.starts_with("supervision/") {
        "conformance/c4/supervision.json"
    } else if id == "net/http/listen" {
        "conformance/c5/http-serving.json"
    } else if id == "net/http/fetch" {
        "conformance/c4/http-client.json"
    } else if matches!(
        id,
        "net/wifi/access-point" | "net/dhcp/server" | "net/reachability" | "net/dns-sd"
    ) {
        "conformance/c4/netherwick-network.json"
    } else if matches!(
        id,
        "conduit.media/wave/literal"
            | "conduit.media/container/probe"
            | "conduit.media/container/demux"
            | "conduit.media/container/mux"
            | "conduit.media/audio/decode"
            | "conduit.media/audio/encode"
    ) {
        "conformance/c4/media-codecs.json"
    } else if id.starts_with("learned/") {
        "conformance/c4/learned-inference.json"
    } else if id.starts_with("spatial/") {
        "conformance/c4/spatial-foundation.json"
    } else if id.starts_with("conduit.media/") {
        "conformance/c4/media-values.json"
    } else if classification == "optional-host-boundary" {
        "conformance/c5/registry-availability.json"
    } else {
        "conformance/c4/standard-node-library.json"
    }
}

fn lesson(id: &str, composition: bool) -> Lesson {
    let published = match (id, composition) {
        ("std/literal" | "io/stdout", false) => Some("welcome.hello-panel"),
        ("std/literal" | "io/stdout", true) => Some("platform.cross-host-provider-conformance"),
        (
            "std/text/format" | "std/format-values/literal" | "std/text/lines" | "std/text/join",
            _,
        ) => Some("library.typed-text-format"),
        (
            "conduit.std/tee" | "conduit.std/merge" | "conduit.std/zip" | "conduit.std/gate"
            | "conduit.std/select",
            _,
        ) => Some("library.standard-flow-control"),
        ("time/delay" | "time/timeout" | "time/debounce" | "time/throttle", _) => {
            Some("library.explicit-time")
        }
        ("state/cell" | "state/deduplicate" | "state/cache", _) => Some("library.bounded-state"),
        ("supervision/retry" | "supervision/circuit-breaker", _) => {
            Some("library.bounded-supervision")
        }
        (
            "std/record/literal"
            | "std/data/encode-utf8"
            | "std/data/decode-utf8"
            | "std/data/frame-length-u32be"
            | "std/data/deframe-length-u32be"
            | "std/data/validate-closed-record"
            | "std/testing/assert-validation-decision",
            _,
        ) => Some("library.explicit-data-boundaries"),
        ("fs/chunk/literal" | "fs/read" | "fs/write" | "fs/watch", _) => {
            Some("library.bounded-filesystem")
        }
        (
            "conduit.host/process/exec"
            | "io/stdin-stream"
            | "io/stdout-stream"
            | "io/stderr-stream",
            _,
        ) => Some("library.bounded-process-exec"),
        (
            "conduit.host/net/tcp/connect"
            | "conduit.host/net/tcp/listen"
            | "conduit.host/net/udp/connected"
            | "conduit.host/net/udp/datagram",
            _,
        ) => Some("library.bounded-sockets"),
        ("net/http/request/literal" | "net/http/fetch", _) => Some("library.bounded-http-client"),
        ("net/wifi/access-point" | "net/dhcp/server" | "net/reachability" | "net/dns-sd", _) => {
            Some("library.bounded-brainstem-network")
        }
        (
            "storage/blob/literal"
            | "storage/cache/put"
            | "storage/cache/get"
            | "storage/cache/remove",
            _,
        ) => Some("library.evictable-storage-cache"),
        (
            "conduit.media/wave/literal"
            | "conduit.media/container/probe"
            | "conduit.media/container/demux"
            | "conduit.media/container/mux"
            | "conduit.media/audio/decode"
            | "conduit.media/audio/encode",
            _,
        ) => Some("library.bounded-media-codecs"),
        (id, _) if id.starts_with("learned/") => Some("library.bounded-learned-inference"),
        (id, _) if id.starts_with("spatial/") => Some("library.bounded-spatial-foundation"),
        (id, _) if id.starts_with("conduit.media/") => Some("library.bounded-media-values"),
        ("text/uppercase", true) => Some("panels.put-a-panel-in-a-panel"),
        _ => None,
    };
    if let Some(artifact) = published {
        return Lesson {
            artifact: format!("tour/lessons/current.json#{artifact}"),
            status: "published",
        };
    }
    let slug = id.replace('/', ".");
    Lesson {
        artifact: format!(
            "tour/lessons/current.json#library.{slug}.{}",
            if composition {
                "composition"
            } else {
                "standalone"
            }
        ),
        status: "required",
    }
}

fn validate_semantic_port_inventory(entries: &[Entry]) -> Result<(), String> {
    const DISPLACED: &[&str] = &[
        "in", "out", "input", "output", "in1", "in2", "out1", "out2", "control",
    ];
    for entry in entries {
        for port in &entry.ports {
            if DISPLACED.contains(&port.id.as_str()) {
                return Err(format!(
                    "`{}.{}` retains a displaced directional or positional identity",
                    entry.semantic_identity, port.id
                ));
            }
        }
    }
    Ok(())
}

fn build() -> Result<Inventory, Box<dyn std::error::Error>> {
    let mut registry = Registry::default();
    conduit_media::register_media_contracts(&mut registry);
    conduit_media::register_media_codec_contracts(&mut registry);
    conduit_learned::register_learned_contracts(&mut registry);
    conduit_spatial::register_spatial_contracts(&mut registry);
    conduit_net::register_network_contracts(&mut registry);
    let standard = conduit_std::STANDARD_CATALOG
        .iter()
        .map(|entry| entry.contract.id.as_str())
        .collect::<BTreeSet<_>>();
    let compiler = builtin_catalog_document()?
        .nodes
        .into_iter()
        .map(|pin| pin.id)
        .collect::<BTreeSet<_>>();
    let mut providers = BTreeMap::<&str, Vec<Provider>>::new();
    for provider in Registry::installed_hosted_providers() {
        providers
            .entry(provider.contract.id.as_str())
            .or_default()
            .push(Provider {
                implementation: provider.manifest.id.to_string(),
                artifact: provider.artifact.id.to_string(),
                artifact_digest: provider.artifact.digest.to_string(),
            });
    }

    let mut entries = Vec::new();
    let mut identities = BTreeSet::new();
    for contract in registry.contracts() {
        let id = contract.id.as_str();
        if !identities.insert(id) {
            return Err(format!("duplicate active semantic identity `{id}`").into());
        }
        let owner = ownership(id)?;
        let schema = OwnedNodeSchema::from_contract(contract);
        let ports = contract
            .inputs
            .iter()
            .chain(contract.outputs.iter())
            .map(|port| {
                let direction = port.direction.as_str();
                let port_id = port.id.as_str();
                let value_type = port.value_type.contract_id.as_str();
                let type_hash = port.value_type.semantic_hash.to_string();
                let presence = match port.presence {
                    conduit_core::Presence::Required => "required",
                    conduit_core::Presence::Optional => "optional",
                };
                Port {
                    id: port_id.to_owned(),
                    direction,
                    value_type: value_type.to_owned(),
                    type_schema_version: port.value_type.schema_version,
                    type_hash,
                    presence,
                    connections: port.connections.as_str(),
                    values: port.values.as_str(),
                    delivery: port.delivery.as_str(),
                    temporal: port.temporal.as_str(),
                    terminal: port.terminal.as_str(),
                    sensitivity: port.sensitivity.as_str(),
                    loss: port.flow.loss.as_str(),
                }
            })
            .collect();
        let config = contract
            .config
            .fields
            .iter()
            .map(|field| ConfigField {
                key: field.key.to_string(),
                value_type: field.value_type.contract_id.to_string(),
                type_schema_version: field.value_type.schema_version,
                type_hash: field.value_type.semantic_hash.to_string(),
            })
            .collect();
        entries.push(Entry {
            semantic_identity: id.to_owned(),
            public_source_spelling: id.to_owned(),
            classification: owner.classification,
            package_owner: owner.package_owner,
            contract_package_artifact: format!("conduit.contract-package/{}", owner.package_owner),
            export_path: format!("{}/{}", owner.package_owner, id),
            schema_version: 0,
            semantic_hash: schema.semantic_hash().to_string(),
            ports,
            config,
            catalog_membership: if standard.contains(id) {
                "portable-catalog"
            } else {
                "host-registry"
            },
            compiler_exported: compiler.contains(id),
            known_provider_bundles: providers.remove(id).unwrap_or_default(),
            current_provider_observation: "not-recorded-in-catalog",
            conformance_fixture_owner: fixture(id, owner.classification),
            required_result_profile: "conduit.cross-host-provider",
            structural_facet_owner: format!("{}/facets", owner.package_owner),
            standalone_lesson: lesson(id, false),
            composition_lesson: lesson(id, true),
            successor: None,
            deprecation: None,
            compatible_adapter_artifacts: Vec::new(),
        });
    }
    entries.sort_by(|left, right| left.semantic_identity.cmp(&right.semantic_identity));
    validate_semantic_port_inventory(&entries)?;
    if !providers.is_empty() {
        return Err("installed provider refers to an unknown catalog contract".into());
    }

    Ok(Inventory {
        schema: "conduit.library-catalog",
        schema_version: 0,
        source_lowering_rule: SourceLoweringRule {
            rule: "public source spelling equals canonical semantic identity",
            aliases_active: false,
        },
        entries,
    })
}

fn render_index(inventory: &Inventory) -> String {
    let mut output = String::from(
        "# Library and Tour index\n\n\
         This file is generated from the exact published registry by \
         `cargo xtask catalog-index`. Catalog membership records known meaning; \
         it does not claim a current provider observation or grant host authority.\n\n\
         | Contract | Class | Package | Provider bundles | Standalone lesson | Composition lesson |\n\
         |---|---|---|---:|---|---|\n",
    );
    for entry in &inventory.entries {
        output.push_str(&format!(
            "| `{}` | {} | `{}` | {} | {} ({}) | {} ({}) |\n",
            entry.semantic_identity,
            entry.classification,
            entry.package_owner,
            entry.known_provider_bundles.len(),
            entry.standalone_lesson.artifact,
            entry.standalone_lesson.status,
            entry.composition_lesson.artifact,
            entry.composition_lesson.status,
        ));
    }
    output
}

pub fn run(workspace_root: &Path, check: bool) -> Result<(), Box<dyn std::error::Error>> {
    let inventory = build()?;
    for entry in &inventory.entries {
        if !workspace_root
            .join(entry.conformance_fixture_owner)
            .is_file()
        {
            return Err(format!(
                "`{}` has missing fixture owner `{}`",
                entry.semantic_identity, entry.conformance_fixture_owner
            )
            .into());
        }
    }
    let inventory_bytes = serde_json::to_vec_pretty(&inventory)?;
    let index = render_index(&inventory);
    let inventory_path = workspace_root.join(INVENTORY_PATH);
    let index_path = workspace_root.join(INDEX_PATH);
    if check {
        if fs::read(&inventory_path).ok().as_deref() != Some(inventory_bytes.as_slice()) {
            return Err(format!("{INVENTORY_PATH} is stale; run cargo xtask catalog-index").into());
        }
        if fs::read_to_string(&index_path).ok().as_deref() != Some(index.as_str()) {
            return Err(format!("{INDEX_PATH} is stale; run cargo xtask catalog-index").into());
        }
    } else {
        if let Some(parent) = inventory_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(inventory_path, inventory_bytes)?;
        fs::write(index_path, index)?;
    }
    println!(
        "library catalog is exact: {} entries, provider facts separate from observations",
        inventory.entries.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_policy_distinguishes_all_catalog_classes() {
        assert_eq!(
            ownership("conduit.std/tee").unwrap().classification,
            "portable-standard"
        );
        assert_eq!(
            ownership("fs/read").unwrap().classification,
            "optional-host-boundary"
        );
        assert_eq!(
            ownership("speech/synthesize").unwrap().classification,
            "reusable-domain-package"
        );
        assert_eq!(
            ownership("test/probe").unwrap().classification,
            "implementation-helper"
        );
        assert!(ownership("unknown/contract").is_err());
    }

    #[test]
    fn exact_inventory_has_no_duplicate_or_ambient_provider_claim() {
        let inventory = build().unwrap();
        let identities = inventory
            .entries
            .iter()
            .map(|entry| entry.semantic_identity.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(identities.len(), inventory.entries.len());
        assert!(
            inventory
                .entries
                .iter()
                .all(|entry| entry.current_provider_observation == "not-recorded-in-catalog")
        );
        assert!(
            inventory
                .entries
                .iter()
                .any(|entry| entry.known_provider_bundles.is_empty())
        );
    }
}
