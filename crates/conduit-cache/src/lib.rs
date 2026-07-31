//! Explicit bounded hosted provider for the evictable blob-cache contracts.

use std::sync::Mutex;

use conduit_core::SemanticHash;
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};
use conduit_std::{
    CACHE_MAX_BLOB_BYTES, CACHE_MAX_ENTRIES, CacheHandle, CacheSensitivity, CacheStore, GetOutcome,
    GetRequest, PutRequest, RemoveOutcome,
};

pub const EXAMPLE_CACHE_PUT_RESOURCE: &str = "conduit.resource/storage-cache-example-put";
pub const EXAMPLE_CACHE_GET_RESOURCE: &str = "conduit.resource/storage-cache-example-get";
pub const EXAMPLE_CACHE_REMOVE_RESOURCE: &str = "conduit.resource/storage-cache-example-remove";
pub const EXAMPLE_CACHE_DESCRIPTOR: &str = "conduit.descriptor/storage-cache-example";
pub const EXAMPLE_PROVIDER_EPOCH: u64 = 0x4341_4348_4500_0001;
pub const EXAMPLE_MAX_RETENTION_TICKS: u64 = 1024;

type HostedCache = CacheStore<CACHE_MAX_ENTRIES, CACHE_MAX_BLOB_BYTES>;

static CACHE: Mutex<HostedCache> = Mutex::new(CacheStore::new(
    EXAMPLE_PROVIDER_EPOCH,
    CACHE_MAX_ENTRIES * CACHE_MAX_BLOB_BYTES,
    EXAMPLE_MAX_RETENTION_TICKS,
    CacheSensitivity::Restricted,
));

fn contract(id: &str) -> &'static conduit_core::NodeContract<'static> {
    conduit_std::standard_node_contract(id).expect("storage cache contract is published")
}

fn validate_exact_keys(node: &Node, expected: &[&str]) -> Result<(), ResolutionError> {
    if node.config.len() != expected.len()
        || expected
            .iter()
            .any(|key| !node.config.iter().any(|entry| entry.key == *key))
    {
        return Err(ResolutionError::new(
            "CND-CACHE-012",
            format!("cache node `{}` does not match its exact config", node.id),
        ));
    }
    Ok(())
}

fn exact_secret(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::SecretReference(value)) if value == expected
    )
}

fn validate_shared(node: &Node, resource: &str, grant: &str) -> Result<(), ResolutionError> {
    if !exact_secret(node, "resource", resource)
        || !exact_secret(node, "grant", grant)
        || node.config("descriptor") != Some(EXAMPLE_CACHE_DESCRIPTOR)
        || node.config("cancellation") != Some("discard")
    {
        return Err(ResolutionError::new(
            "CND-CACHE-012",
            format!("cache node `{}` has unsupported provider facts", node.id),
        ));
    }
    Ok(())
}

fn validate_positive_bound(node: &Node, key: &str, maximum: u64) -> Result<(), ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) if *value > 0 && *value <= maximum as i128 => Ok(()),
        _ => Err(ResolutionError::new(
            "CND-CACHE-001",
            format!("cache node `{}` has invalid `{key}`", node.id),
        )),
    }
}

fn validate_blob_literal(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(node, &["value", "maximum_bytes"])?;
    validate_positive_bound(node, "maximum_bytes", CACHE_MAX_BLOB_BYTES as u64)?;
    let maximum = required_usize_resolution(node, "maximum_bytes")?;
    if !matches!(
        node.config_value("value"),
        Some(SourceValue::Bytes(bytes)) if bytes.len() <= maximum
    ) {
        return Err(ResolutionError::new(
            "CND-CACHE-004",
            format!("blob literal `{}` exceeds its exact bound", node.id),
        ));
    }
    Ok(())
}

fn validate_put(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "descriptor",
            "run_epoch",
            "now_tick",
            "retention_ticks",
            "maximum_blob_bytes",
            "persistence",
            "eviction",
            "sensitivity",
            "cancellation",
        ],
    )?;
    validate_shared(
        node,
        EXAMPLE_CACHE_PUT_RESOURCE,
        "conduit.grant/storage-cache-put",
    )?;
    validate_positive_bound(node, "retention_ticks", EXAMPLE_MAX_RETENTION_TICKS)?;
    validate_positive_bound(node, "maximum_blob_bytes", CACHE_MAX_BLOB_BYTES as u64)?;
    if node.config("persistence") != Some("evictable")
        || node.config("eviction") != Some("fifo")
        || !matches!(node.config("sensitivity"), Some("public" | "restricted"))
        || !matches!(
            node.config_value("run_epoch"),
            Some(SourceValue::Integer(value)) if *value >= 0 && u64::try_from(*value).is_ok()
        )
        || !matches!(
            node.config_value("now_tick"),
            Some(SourceValue::Integer(value)) if *value >= 0 && u64::try_from(*value).is_ok()
        )
    {
        return Err(ResolutionError::new(
            "CND-CACHE-012",
            format!("cache put `{}` has unsupported semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_get(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "descriptor",
            "run_epoch",
            "now_tick",
            "maximum_blob_bytes",
            "integrity",
            "cancellation",
        ],
    )?;
    validate_shared(
        node,
        EXAMPLE_CACHE_GET_RESOURCE,
        "conduit.grant/storage-cache-get",
    )?;
    validate_positive_bound(node, "maximum_blob_bytes", CACHE_MAX_BLOB_BYTES as u64)?;
    if node.config("integrity") != Some("sha256-before-yield")
        || required_u64_resolution(node, "run_epoch").is_err()
        || required_u64_resolution(node, "now_tick").is_err()
    {
        return Err(ResolutionError::new(
            "CND-CACHE-012",
            format!("cache get `{}` has unsupported semantics", node.id),
        ));
    }
    Ok(())
}

fn validate_remove(node: &Node) -> Result<(), ResolutionError> {
    validate_exact_keys(
        node,
        &[
            "resource",
            "grant",
            "descriptor",
            "run_epoch",
            "cancellation",
        ],
    )?;
    validate_shared(
        node,
        EXAMPLE_CACHE_REMOVE_RESOURCE,
        "conduit.grant/storage-cache-remove",
    )?;
    required_u64_resolution(node, "run_epoch").map(|_| ())
}

fn required_usize_resolution(node: &Node, key: &str) -> Result<usize, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => usize::try_from(*value).map_err(|_| {
            ResolutionError::new(
                "CND-CACHE-001",
                format!("cache node `{}` has invalid `{key}`", node.id),
            )
        }),
        _ => Err(ResolutionError::new(
            "CND-CACHE-001",
            format!("cache node `{}` has no exact `{key}`", node.id),
        )),
    }
}

fn required_u64_resolution(node: &Node, key: &str) -> Result<u64, ResolutionError> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).map_err(|_| {
            ResolutionError::new(
                "CND-CACHE-001",
                format!("cache node `{}` has invalid `{key}`", node.id),
            )
        }),
        _ => Err(ResolutionError::new(
            "CND-CACHE-001",
            format!("cache node `{}` has no exact `{key}`", node.id),
        )),
    }
}

fn required_u64(node: &Node, key: &str) -> Result<u64, RuntimeError> {
    required_u64_resolution(node, key).map_err(|error| RuntimeError::new(error.code, error.message))
}

fn required_usize(node: &Node, key: &str) -> Result<usize, RuntimeError> {
    required_usize_resolution(node, key)
        .map_err(|error| RuntimeError::new(error.code, error.message))
}

fn sensitivity(node: &Node) -> Result<CacheSensitivity, RuntimeError> {
    match node.config("sensitivity") {
        Some("public") => Ok(CacheSensitivity::Public),
        Some("restricted") => Ok(CacheSensitivity::Restricted),
        _ => Err(RuntimeError::new(
            "CND-CACHE-003",
            "cache sensitivity disappeared",
        )),
    }
}

fn encode_handle(handle: CacheHandle) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(66);
    bytes.extend_from_slice(&handle.provider_epoch.to_be_bytes());
    bytes.extend_from_slice(&handle.run_epoch.to_be_bytes());
    bytes.extend_from_slice(&handle.slot.to_be_bytes());
    bytes.extend_from_slice(&handle.generation.to_be_bytes());
    bytes.extend_from_slice(&handle.identity.digest);
    bytes.extend_from_slice(&(handle.identity.bytes as u64).to_be_bytes());
    bytes
}

fn decode_handle(bytes: &[u8]) -> Result<CacheHandle, RuntimeError> {
    if bytes.len() != 66 {
        return Err(RuntimeError::new(
            "CND-CACHE-009",
            "cache handle has invalid encoding",
        ));
    }
    let provider_epoch = u64::from_be_bytes(bytes[0..8].try_into().expect("fixed range"));
    let run_epoch = u64::from_be_bytes(bytes[8..16].try_into().expect("fixed range"));
    let slot = u16::from_be_bytes(bytes[16..18].try_into().expect("fixed range"));
    let generation = u64::from_be_bytes(bytes[18..26].try_into().expect("fixed range"));
    let digest = bytes[26..58].try_into().expect("fixed range");
    let length = u64::from_be_bytes(bytes[58..66].try_into().expect("fixed range"));
    Ok(CacheHandle {
        provider_epoch,
        run_epoch,
        slot,
        generation,
        identity: conduit_std::BlobIdentity {
            digest,
            bytes: usize::try_from(length).map_err(|_| {
                RuntimeError::new("CND-CACHE-009", "cache handle length is invalid")
            })?,
        },
    })
}

fn put_result_bytes(result: conduit_std::PutResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(73);
    bytes.extend_from_slice(&result.expires_at_tick.to_be_bytes());
    if let Some(evicted) = result.evicted {
        bytes.push(1);
        bytes.extend_from_slice(&evicted.digest);
        bytes.extend_from_slice(&(evicted.bytes as u64).to_be_bytes());
    } else {
        bytes.push(0);
    }
    bytes
}

fn get_result_bytes(result: conduit_std::GetResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(49);
    bytes.push(match result.outcome {
        GetOutcome::Hit => 0,
        GetOutcome::Miss => 1,
        GetOutcome::Evicted => 2,
        GetOutcome::Expired => 3,
    });
    bytes.extend_from_slice(&result.identity.digest);
    bytes.extend_from_slice(&(result.identity.bytes as u64).to_be_bytes());
    bytes.extend_from_slice(&(result.bytes_read as u64).to_be_bytes());
    bytes
}

struct BlobLiteral;

impl Handler for BlobLiteral {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(RuntimeError::new(
                "CND-CACHE-012",
                "blob literal received hidden inputs",
            ));
        }
        validate_blob_literal(node)
            .map_err(|error| RuntimeError::new(error.code, error.message))?;
        let Some(SourceValue::Bytes(bytes)) = node.config_value("value") else {
            return Err(RuntimeError::new(
                "CND-CACHE-004",
                "blob literal value disappeared",
            ));
        };
        Ok(vec![Value {
            value_type: contract("storage/blob/literal").outputs[0].value_type,
            bytes: bytes.clone(),
        }])
    }
}

struct PutHandler;

impl Handler for PutHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_put(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let contract = contract("storage/cache/put");
        let input = inputs
            .first()
            .filter(|input| input.value_type == contract.inputs[0].value_type)
            .ok_or_else(|| RuntimeError::new("CND-CACHE-012", "cache put blob is missing"))?;
        let result = CACHE
            .lock()
            .map_err(|_| RuntimeError::new("CND-CACHE-006", "cache provider lock failed"))?
            .put(
                PutRequest {
                    run_epoch: required_u64(node, "run_epoch")?,
                    now_tick: required_u64(node, "now_tick")?,
                    retention_ticks: required_u64(node, "retention_ticks")?,
                    maximum_blob_bytes: required_usize(node, "maximum_blob_bytes")?,
                    sensitivity: sensitivity(node)?,
                },
                &input.bytes,
            )
            .map_err(cache_error)?;
        Ok(vec![
            Value {
                value_type: contract.outputs[0].value_type,
                bytes: encode_handle(result.handle),
            },
            Value {
                value_type: contract.outputs[1].value_type,
                bytes: put_result_bytes(result),
            },
        ])
    }
}

struct GetHandler;

impl Handler for GetHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_get(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let contract = contract("storage/cache/get");
        let input = inputs
            .first()
            .filter(|input| input.value_type == contract.inputs[0].value_type)
            .ok_or_else(|| RuntimeError::new("CND-CACHE-012", "cache get handle is missing"))?;
        let maximum = required_usize(node, "maximum_blob_bytes")?;
        let mut output = vec![0; maximum];
        let result = CACHE
            .lock()
            .map_err(|_| RuntimeError::new("CND-CACHE-006", "cache provider lock failed"))?
            .get(
                GetRequest {
                    run_epoch: required_u64(node, "run_epoch")?,
                    now_tick: required_u64(node, "now_tick")?,
                    maximum_blob_bytes: maximum,
                    handle: decode_handle(&input.bytes)?,
                },
                &mut output,
            )
            .map_err(cache_error)?;
        output.truncate(result.bytes_read);
        Ok(vec![
            Value {
                value_type: contract.outputs[0].value_type,
                bytes: output,
            },
            Value {
                value_type: contract.outputs[1].value_type,
                bytes: get_result_bytes(result),
            },
        ])
    }
}

struct RemoveHandler;

impl Handler for RemoveHandler {
    fn run(
        &mut self,
        node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        validate_remove(node).map_err(|error| RuntimeError::new(error.code, error.message))?;
        let contract = contract("storage/cache/remove");
        let input = inputs
            .first()
            .filter(|input| input.value_type == contract.inputs[0].value_type)
            .ok_or_else(|| RuntimeError::new("CND-CACHE-012", "cache remove handle is missing"))?;
        let outcome = CACHE
            .lock()
            .map_err(|_| RuntimeError::new("CND-CACHE-006", "cache provider lock failed"))?
            .remove(
                required_u64(node, "run_epoch")?,
                decode_handle(&input.bytes)?,
            )
            .map_err(cache_error)?;
        Ok(vec![Value {
            value_type: contract.outputs[0].value_type,
            bytes: vec![match outcome {
                RemoveOutcome::Removed => 0,
                RemoveOutcome::Missing => 1,
            }],
        }])
    }
}

fn cache_error(error: conduit_std::CacheError) -> RuntimeError {
    RuntimeError::new(error.code(), error.code())
}

fn reset_cache() -> Result<(), RegistryError> {
    let mut cache = CACHE.lock().map_err(|_| RegistryError {
        code: "CND-REG-008",
        message: "cache provider lock failed".to_owned(),
    })?;
    *cache = CacheStore::new(
        EXAMPLE_PROVIDER_EPOCH,
        CACHE_MAX_ENTRIES * CACHE_MAX_BLOB_BYTES,
        EXAMPLE_MAX_RETENTION_TICKS,
        CacheSensitivity::Restricted,
    );
    Ok(())
}

/// Explicitly installs the complete bounded example cache provider bundle.
pub fn register_hosted_cache_provider(registry: &mut Registry) -> Result<(), RegistryError> {
    reset_cache()?;
    static PUT_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x41; 32])];
    static GET_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x42; 32])];
    static REMOVE_AUTHORITY: [SemanticHash; 1] = [SemanticHash::from_bytes([0x43; 32])];
    for service in [
        CompiledInHostService {
            contract: contract("storage/blob/literal"),
            implementation_id: "conduit/storage-blob-literal",
            artifact_id: "conduit/storage-cache-artifact",
            entrypoint: "storage-blob-literal",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &[],
            factory: || Box::new(BlobLiteral),
            validate_config: validate_blob_literal,
        },
        CompiledInHostService {
            contract: contract("storage/cache/put"),
            implementation_id: "conduit/storage-cache-put",
            artifact_id: "conduit/storage-cache-artifact",
            entrypoint: "storage-cache-put",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &PUT_AUTHORITY,
            factory: || Box::new(PutHandler),
            validate_config: validate_put,
        },
        CompiledInHostService {
            contract: contract("storage/cache/get"),
            implementation_id: "conduit/storage-cache-get",
            artifact_id: "conduit/storage-cache-artifact",
            entrypoint: "storage-cache-get",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &GET_AUTHORITY,
            factory: || Box::new(GetHandler),
            validate_config: validate_get,
        },
        CompiledInHostService {
            contract: contract("storage/cache/remove"),
            implementation_id: "conduit/storage-cache-remove",
            artifact_id: "conduit/storage-cache-artifact",
            entrypoint: "storage-cache-remove",
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &REMOVE_AUTHORITY,
            factory: || Box::new(RemoveHandler),
            validate_config: validate_remove,
        },
    ] {
        registry.register_compiled_in_host_service(service)?;
    }
    Ok(())
}

/// Redacted provider description suitable for host reports and Patchbay.
#[must_use]
pub fn provider_description() -> Vec<(&'static str, String)> {
    vec![
        ("availability", "best-effort".to_owned()),
        ("persistence", "evictable".to_owned()),
        ("eviction", "fifo".to_owned()),
        ("integrity", "sha256-before-yield".to_owned()),
        ("maximum_entries", CACHE_MAX_ENTRIES.to_string()),
        ("maximum_blob_bytes", CACHE_MAX_BLOB_BYTES.to_string()),
        (
            "maximum_total_bytes",
            (CACHE_MAX_ENTRIES * CACHE_MAX_BLOB_BYTES).to_string(),
        ),
        (
            "maximum_retention_ticks",
            EXAMPLE_MAX_RETENTION_TICKS.to_string(),
        ),
        ("accepted_sensitivity", "public,restricted".to_owned()),
        ("descriptor", EXAMPLE_CACHE_DESCRIPTOR.to_owned()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_runtime::AvailabilityState;

    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    fn run_handler(
        handler: &mut dyn Handler,
        node: &Node,
        inputs: &[Value],
    ) -> Result<Vec<Value>, RuntimeError> {
        let mut input = std::io::empty();
        let mut output = Vec::new();
        let mut error = Vec::new();
        let mut display = Vec::new();
        handler.run(
            node,
            inputs,
            &mut RunIo {
                input: &mut input,
                output: &mut output,
                error: &mut error,
                display: &mut display,
            },
        )
    }

    #[test]
    fn provider_is_absent_until_explicit_bundle_installation() {
        let _serial = TEST_SERIAL.lock().unwrap();
        let default = Registry::default();
        assert_eq!(
            default.node_availability("storage/cache/put").state,
            AvailabilityState::ContractOnly
        );
        let mut registry = Registry::default();
        register_hosted_cache_provider(&mut registry).unwrap();
        for id in [
            "storage/blob/literal",
            "storage/cache/put",
            "storage/cache/get",
            "storage/cache/remove",
        ] {
            assert_eq!(
                registry.node_availability(id).state,
                AvailabilityState::ProviderAvailable
            );
        }
    }

    #[test]
    fn description_is_bounded_best_effort_and_redacted() {
        let description = provider_description();
        assert!(description.len() <= 10);
        assert!(description.contains(&("persistence", "evictable".to_owned())));
        let rendered = format!("{description:?}");
        assert!(!rendered.contains("grant"));
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("path"));
    }

    #[test]
    fn exact_handle_encoding_rejects_provider_and_run_drift() {
        let _serial = TEST_SERIAL.lock().unwrap();
        reset_cache().unwrap();
        let result = CACHE
            .lock()
            .unwrap()
            .put(
                PutRequest {
                    run_epoch: 9,
                    now_tick: 1,
                    retention_ticks: 10,
                    maximum_blob_bytes: 16,
                    sensitivity: CacheSensitivity::Public,
                },
                b"cache",
            )
            .unwrap();
        assert_eq!(
            decode_handle(&encode_handle(result.handle)).unwrap(),
            result.handle
        );
        let mut wrong = result.handle;
        wrong.run_epoch = 10;
        assert_eq!(
            CACHE.lock().unwrap().remove(9, wrong),
            Err(conduit_std::CacheError::WrongRun)
        );
    }

    #[test]
    fn deterministic_and_hosted_providers_agree_on_normalized_cache_outcomes() {
        let _serial = TEST_SERIAL.lock().unwrap();
        reset_cache().unwrap();
        let panel = conduit_panel::parse(include_str!("../../../examples/storage-cache.panel"))
            .expect("cache panel parses");
        let put_node = panel
            .nodes
            .iter()
            .find(|node| node.id == "put")
            .expect("put node");
        let get_node = panel
            .nodes
            .iter()
            .find(|node| node.id == "get")
            .expect("get node");
        let mut reference = HostedCache::new(
            EXAMPLE_PROVIDER_EPOCH,
            CACHE_MAX_ENTRIES * CACHE_MAX_BLOB_BYTES,
            EXAMPLE_MAX_RETENTION_TICKS,
            CacheSensitivity::Restricted,
        );
        let request = PutRequest {
            run_epoch: 1,
            now_tick: 10,
            retention_ticks: 100,
            maximum_blob_bytes: 64,
            sensitivity: CacheSensitivity::Restricted,
        };
        let mut handles = Vec::new();
        for bytes in [
            b"first".as_slice(),
            b"second",
            b"third",
            b"fourth",
            b"fifth",
        ] {
            let expected = reference.put(request, bytes).unwrap();
            let actual = run_handler(
                &mut PutHandler,
                put_node,
                &[Value {
                    value_type: contract("storage/cache/put").inputs[0].value_type,
                    bytes: bytes.to_vec(),
                }],
            )
            .unwrap();
            assert_eq!(decode_handle(&actual[0].bytes).unwrap(), expected.handle);
            assert_eq!(actual[1].bytes, put_result_bytes(expected));
            handles.push(expected.handle);
        }

        for handle in [handles[4], handles[0]] {
            let mut bytes = [0; 64];
            let expected = reference
                .get(
                    GetRequest {
                        run_epoch: 1,
                        now_tick: 11,
                        maximum_blob_bytes: 64,
                        handle,
                    },
                    &mut bytes,
                )
                .unwrap();
            let actual = run_handler(
                &mut GetHandler,
                get_node,
                &[Value {
                    value_type: contract("storage/cache/get").inputs[0].value_type,
                    bytes: encode_handle(handle),
                }],
            )
            .unwrap();
            assert_eq!(actual[0].bytes, bytes[..expected.bytes_read]);
            assert_eq!(actual[1].bytes, get_result_bytes(expected));
        }

        reference.set_available(false);
        CACHE.lock().unwrap().set_available(false);
        let expected = reference
            .get(
                GetRequest {
                    run_epoch: 1,
                    now_tick: 11,
                    maximum_blob_bytes: 64,
                    handle: handles[4],
                },
                &mut [0; 64],
            )
            .unwrap_err();
        let actual = run_handler(
            &mut GetHandler,
            get_node,
            &[Value {
                value_type: contract("storage/cache/get").inputs[0].value_type,
                bytes: encode_handle(handles[4]),
            }],
        )
        .unwrap_err();
        assert_eq!(actual.code, expected.code());
        reset_cache().unwrap();
    }
}
