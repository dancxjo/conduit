#![cfg(feature = "confined-gear")]

use conduit_core::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseCapabilityAuthority,
    BaseCapabilityRefusal, BaseCapabilityScope, BaseCapabilityTable, BaseInstanceId,
    BaseOperationClaim, BootId, CapabilityEnvelopeId, CapabilityId, CapabilityIssueRequest, HostId,
    HostOperationContractId, ImplementationId, KindId, PlanId, ResourceGenerationId,
    ResourcePoolId,
};
use conduit_std_host::confined_gear::{
    execute, operation_tag, prepare, AllowedImports, ConfinedAuthority, ConfinedBaseProvider,
    ConfinedLimits, ConfinedRefusal, RealizationClass,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const SLOT: u32 = 7;
const RESOURCE: u64 = 0xfedc_ba98_7654_3210;

struct RecordingProvider {
    calls: usize,
    expected: &'static [u8],
}

struct FileProvider {
    allowed: PathBuf,
}

impl ConfinedBaseProvider for FileProvider {
    fn call(&mut self, request: &[u8]) -> Result<u32, BaseCapabilityRefusal> {
        fs::write(&self.allowed, request).map_err(|_| BaseCapabilityRefusal::WrongScope)?;
        Ok(0)
    }
}

impl ConfinedBaseProvider for RecordingProvider {
    fn call(&mut self, request: &[u8]) -> Result<u32, BaseCapabilityRefusal> {
        assert_eq!(request, self.expected);
        self.calls += 1;
        Ok(0)
    }
}

fn wasm(source: &str) -> Vec<u8> {
    wat::parse_str(source).unwrap()
}

fn prepared(
    source: &str,
    imports: AllowedImports,
) -> conduit_std_host::confined_gear::PreparedArtifact {
    let bytes = wasm(source);
    let digest = Sha256::digest(&bytes).into();
    prepare(&bytes, digest, imports, ConfinedLimits::baseline()).unwrap()
}

fn issue() -> CapabilityIssueRequest {
    let operation = HostOperationContractId::from("conduit.host/confined-write@1");
    let scope = BaseCapabilityScope {
        host_id: HostId::from("host/confined"),
        boot_id: BootId::from("boot/current"),
        base_instance_id: BaseInstanceId::from("base/confined/provider"),
        base_provider_generation: 1,
        plan_id: PlanId::from("plan/confined"),
        active_play_id: ActivePlayId::from("play/confined"),
        authority_grant_id: AuthorityGrantId::from("grant/confined"),
        authority_contract_id: AuthorityContractId::from("authority/confined@1"),
        capability_id: CapabilityId::from("file/write"),
        implementation_id: ImplementationId::from("wasm/confined@1"),
        operation_contract_id: operation.clone(),
        subject_kind: KindId::from("file/allowed"),
        resource_pool_id: ResourcePoolId::from("proof/allowed"),
        resource_generation_id: ResourceGenerationId("allowed/generation/1".into()),
        envelope_id: CapabilityEnvelopeId::from("file/write/allowed/max-64"),
        maximum_parameter_bytes: 64,
        maximum_result_bytes: 1,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 1,
    };
    CapabilityIssueRequest {
        authority: BaseCapabilityAuthority {
            grant: AuthorityGrant {
                grant_id: scope.authority_grant_id.clone(),
                contract_id: scope.authority_contract_id.clone(),
                host_operation_contract_id: operation.clone(),
                subject_kind: scope.subject_kind.clone(),
                host_id: scope.host_id.clone(),
                boot_id: scope.boot_id.clone(),
                capability_id: scope.capability_id.clone(),
            },
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: operation,
            envelope_id: scope.envelope_id.clone(),
            maximum_parameter_bytes: 64,
            maximum_result_bytes: 1,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        },
        scope,
    }
}

fn authority() -> ConfinedAuthority {
    let issue = issue();
    let scope = &issue.scope;
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: 0,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [3; 32],
        1,
    )
    .unwrap();
    let handle = table.issue(issue).unwrap();
    ConfinedAuthority {
        slot: SLOT,
        resource_tag: RESOURCE,
        table,
        handle,
        claim,
    }
}

fn effect_module(
    slot: u32,
    operation: u32,
    resource: u64,
    pointer: i32,
    length: i32,
    calls: usize,
) -> String {
    let calls = (0..calls)
        .map(|_| format!("i32.const {slot} i64.const {operation} i64.const {} i32.const {pointer} i32.const {length} call $effect drop", resource as i64))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"(module
      (import "conduit" "effect_call" (func $effect (param i32 i64 i64 i32 i32) (result i32)))
      (memory (export "memory") 1 1)
      (data (i32.const 0) "authorized")
      (func (export "conduit_run") (param i32) (result i32) {calls} i32.const 0))"#
    )
}

#[test]
fn pure_profile_has_an_empty_import_inventory_and_finite_execution() {
    let artifact = prepared(
        r#"(module (memory (export "memory") 1 1)
            (data (i32.const 32768) "PURE")
            (func (export "conduit_run") (param i32) (result i32) i32.const 4))"#,
        AllowedImports::Pure,
    );
    let execution = execute(
        &artifact,
        b"typed-input",
        None,
        RecordingProvider {
            calls: 0,
            expected: b"",
        },
    )
    .unwrap();
    assert_eq!(execution.output, b"PURE");
    assert!(execution.imports.is_empty());
    assert_eq!(execution.memory_bytes, 64 * 1024);
    assert!(execution.fuel_consumed > 0);
    assert_eq!(execution.host_calls, 0);
    assert_eq!(
        execution.realization_class,
        RealizationClass::ConfinedThirdParty
    );
}

#[test]
fn preparation_binds_digest_imports_and_fixed_memory_before_play() {
    let forbidden = [
        ("wasi_snapshot_preview1", "fd_read"),
        ("network", "connect"),
        ("process", "spawn"),
        ("device", "open"),
        ("ros", "publish"),
        ("conduit", "host_adapter"),
    ];
    for (module, name) in forbidden {
        let bytes = wasm(&format!(
            r#"(module (import "{module}" "{name}" (func)) (memory (export "memory") 1 1)
                (func (export "conduit_run") (param i32) (result i32) i32.const 0))"#
        ));
        assert!(matches!(
            prepare(
                &bytes,
                Sha256::digest(&bytes).into(),
                AllowedImports::Pure,
                ConfinedLimits::baseline()
            ),
            Err(ConfinedRefusal::UnexpectedImport(_))
        ));
    }

    let bytes = wasm(
        r#"(module (memory (export "memory") 1 1) (func (export "conduit_run") (param i32) (result i32) i32.const 0))"#,
    );
    assert_eq!(
        prepare(
            &bytes,
            [0; 32],
            AllowedImports::Pure,
            ConfinedLimits::baseline()
        )
        .unwrap_err(),
        ConfinedRefusal::ArtifactIdentityMismatch
    );
    let growing = wasm(
        r#"(module (memory (export "memory") 1 2) (func (export "conduit_run") (param i32) (result i32) i32.const 0))"#,
    );
    assert_eq!(
        prepare(
            &growing,
            Sha256::digest(&growing).into(),
            AllowedImports::Pure,
            ConfinedLimits::baseline()
        )
        .unwrap_err(),
        ConfinedRefusal::MemoryLimit
    );
}

#[test]
fn effect_import_reaches_only_the_exact_capability_scoped_base_operation() {
    let authority = authority();
    let source = effect_module(SLOT, operation_tag(&authority.claim), RESOURCE, 0, 10, 1);
    let artifact = prepared(&source, AllowedImports::OneBaseEffect);
    let root = std::env::temp_dir().join(format!(
        "conduit-confined-gear-proof-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let allowed = root.join("allowed.txt");
    let sibling = root.join("protected-sibling.txt");
    fs::write(&allowed, b"before").unwrap();
    fs::write(&sibling, b"protected").unwrap();
    let execution = execute(
        &artifact,
        b"authorized",
        Some(authority),
        FileProvider {
            allowed: allowed.clone(),
        },
    )
    .unwrap();
    assert_eq!(execution.host_calls, 1);
    assert_eq!(execution.imports[0].module, "conduit");
    assert_eq!(execution.imports[0].name, "effect_call");
    assert_eq!(fs::read(&allowed).unwrap(), b"authorized");
    assert_eq!(fs::read(&sibling).unwrap(), b"protected");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn hostile_slots_resources_replay_pointers_fuel_and_call_flood_fail_closed() {
    let cases = [
        (SLOT + 1, RESOURCE, 0, 10, ConfinedRefusal::ForgedSlot),
        (SLOT, RESOURCE + 1, 0, 10, ConfinedRefusal::WrongResource),
        (
            SLOT,
            RESOURCE,
            65_530,
            100,
            ConfinedRefusal::MalformedGuestMemory,
        ),
    ];
    for (slot, resource, pointer, length, expected) in cases {
        let authority = authority();
        let source = effect_module(
            slot,
            operation_tag(&authority.claim),
            resource,
            pointer,
            length,
            1,
        );
        let artifact = prepared(&source, AllowedImports::OneBaseEffect);
        assert_eq!(
            execute(
                &artifact,
                b"authorized",
                Some(authority),
                RecordingProvider {
                    calls: 0,
                    expected: b"authorized"
                }
            )
            .unwrap_err(),
            expected
        );
    }

    let mut revoked = authority();
    revoked.table.revoke(&revoked.handle).unwrap();
    let source = effect_module(SLOT, operation_tag(&revoked.claim), RESOURCE, 0, 10, 1);
    let artifact = prepared(&source, AllowedImports::OneBaseEffect);
    assert_eq!(
        execute(
            &artifact,
            b"authorized",
            Some(revoked),
            RecordingProvider {
                calls: 0,
                expected: b"authorized"
            }
        )
        .unwrap_err(),
        ConfinedRefusal::Capability(BaseCapabilityRefusal::Revoked)
    );

    let authority = authority();
    let source = effect_module(SLOT, operation_tag(&authority.claim), RESOURCE, 0, 10, 2);
    let artifact = prepared(&source, AllowedImports::OneBaseEffect);
    assert_eq!(
        execute(
            &artifact,
            b"authorized",
            Some(authority),
            RecordingProvider {
                calls: 0,
                expected: b"authorized"
            }
        )
        .unwrap_err(),
        ConfinedRefusal::HostCallLimit
    );

    let work = "i32.const 1 drop ".repeat(100);
    let bytes = wasm(&format!(
        r#"(module (memory (export "memory") 1 1) (func (export "conduit_run") (param i32) (result i32) {work} i32.const 0))"#
    ));
    let mut limits = ConfinedLimits::baseline();
    limits.maximum_fuel = 10;
    let spinning = prepare(
        &bytes,
        Sha256::digest(&bytes).into(),
        AllowedImports::Pure,
        limits,
    )
    .unwrap();
    assert_eq!(
        execute(
            &spinning,
            b"",
            None,
            RecordingProvider {
                calls: 0,
                expected: b""
            }
        )
        .unwrap_err(),
        ConfinedRefusal::FuelExhausted
    );
}
