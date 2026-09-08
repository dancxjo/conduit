//! Bounded no-WASI execution for untrusted Gear implementation artifacts.

use conduit_core::{
    BaseCapabilityHandle, BaseCapabilityRefusal, BaseCapabilityTable, BaseOperationClaim,
};
use sha2::{Digest, Sha256};
use wasmi::{
    Caller, Config, Engine, Extern, ExternType, Linker, Module, Store, StoreLimits,
    StoreLimitsBuilder, TrapCode,
};

const EFFECT_MODULE: &str = "conduit";
const EFFECT_NAME: &str = "effect_call";
const INPUT_OFFSET: usize = 0;
const OUTPUT_OFFSET: usize = 32 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealizationClass {
    ConfinedThirdParty,
    TrustedNative,
    LegacyCooperativeNative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfinedLimits {
    pub maximum_artifact_bytes: usize,
    pub maximum_memory_bytes: usize,
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_fuel: u64,
    pub maximum_host_calls: u32,
}

impl ConfinedLimits {
    pub const fn baseline() -> Self {
        Self {
            maximum_artifact_bytes: 64 * 1024,
            maximum_memory_bytes: 64 * 1024,
            maximum_input_bytes: 4 * 1024,
            maximum_output_bytes: 4 * 1024,
            maximum_fuel: 100_000,
            maximum_host_calls: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AllowedImports {
    Pure,
    OneBaseEffect,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImportIdentity {
    pub module: String,
    pub name: String,
}

#[derive(Debug)]
pub struct PreparedArtifact {
    bytes: Vec<u8>,
    pub sha256: [u8; 32],
    pub imports: Vec<ImportIdentity>,
    pub limits: ConfinedLimits,
    pub realization_class: RealizationClass,
}

pub struct ConfinedAuthority {
    pub slot: u32,
    pub resource_tag: u64,
    pub table: BaseCapabilityTable,
    pub handle: BaseCapabilityHandle,
    pub claim: BaseOperationClaim,
}

pub trait ConfinedBaseProvider {
    /// Performs the one admitted operation and returns its finite result size.
    fn call(&mut self, request: &[u8]) -> Result<u32, BaseCapabilityRefusal>;
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ConfinedRefusal {
    ArtifactTooLarge,
    ArtifactIdentityMismatch,
    ArtifactInvalid,
    UnexpectedImport(ImportIdentity),
    MemoryLimit,
    InputLimit,
    OutputLimit,
    FuelExhausted,
    HostCallLimit,
    MissingAuthority,
    ForgedSlot,
    WrongOperation,
    WrongResource,
    MalformedGuestMemory,
    Capability(BaseCapabilityRefusal),
    BaseRefused(BaseCapabilityRefusal),
    ExecutionFailed,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ConfinedExecution {
    pub output: Vec<u8>,
    pub fuel_consumed: u64,
    pub host_calls: u32,
    pub memory_bytes: usize,
    pub imports: Vec<ImportIdentity>,
    pub realization_class: RealizationClass,
}

pub fn prepare(
    bytes: &[u8],
    expected_sha256: [u8; 32],
    allowed: AllowedImports,
    limits: ConfinedLimits,
) -> Result<PreparedArtifact, ConfinedRefusal> {
    validate_limits(limits)?;
    if bytes.len() > limits.maximum_artifact_bytes {
        return Err(ConfinedRefusal::ArtifactTooLarge);
    }
    let digest: [u8; 32] = Sha256::digest(bytes).into();
    if digest != expected_sha256 {
        return Err(ConfinedRefusal::ArtifactIdentityMismatch);
    }
    let engine = engine(limits)?;
    let module = Module::new(&engine, bytes).map_err(|_| ConfinedRefusal::ArtifactInvalid)?;
    let imports = module
        .imports()
        .map(|import| ImportIdentity {
            module: import.module().to_owned(),
            name: import.name().to_owned(),
        })
        .collect::<Vec<_>>();
    for import in &imports {
        let allowed_effect = allowed == AllowedImports::OneBaseEffect
            && import.module == EFFECT_MODULE
            && import.name == EFFECT_NAME;
        if !allowed_effect {
            return Err(ConfinedRefusal::UnexpectedImport(import.clone()));
        }
    }
    if imports.len() > usize::from(allowed == AllowedImports::OneBaseEffect) {
        return Err(ConfinedRefusal::ArtifactInvalid);
    }
    let Some(ExternType::Memory(memory)) = module.get_export("memory") else {
        return Err(ConfinedRefusal::ArtifactInvalid);
    };
    let maximum_pages = memory.maximum().ok_or(ConfinedRefusal::MemoryLimit)?;
    let maximum_bytes = maximum_pages
        .checked_mul(64 * 1024)
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(ConfinedRefusal::MemoryLimit)?;
    if memory.minimum() != maximum_pages || maximum_bytes > limits.maximum_memory_bytes {
        return Err(ConfinedRefusal::MemoryLimit);
    }
    Ok(PreparedArtifact {
        bytes: bytes.to_vec(),
        sha256: digest,
        imports,
        limits,
        realization_class: RealizationClass::ConfinedThirdParty,
    })
}

pub fn execute<P: ConfinedBaseProvider>(
    artifact: &PreparedArtifact,
    input: &[u8],
    authority: Option<ConfinedAuthority>,
    provider: P,
) -> Result<ConfinedExecution, ConfinedRefusal> {
    if input.len() > artifact.limits.maximum_input_bytes {
        return Err(ConfinedRefusal::InputLimit);
    }
    let engine = engine(artifact.limits)?;
    let module =
        Module::new(&engine, &artifact.bytes).map_err(|_| ConfinedRefusal::ArtifactInvalid)?;
    let limits = StoreLimitsBuilder::new()
        .memory_size(artifact.limits.maximum_memory_bytes)
        .memories(1)
        .tables(1)
        .instances(1)
        .build();
    let mut store = Store::new(
        &engine,
        HostState {
            limits,
            authority,
            provider,
            host_calls: 0,
            maximum_host_calls: artifact.limits.maximum_host_calls,
            refusal: None,
        },
    );
    store.limiter(|state| &mut state.limits);
    store
        .set_fuel(artifact.limits.maximum_fuel)
        .map_err(|_| ConfinedRefusal::ExecutionFailed)?;
    let mut linker = Linker::new(&engine);
    if !artifact.imports.is_empty() {
        linker
            .func_wrap(
                EFFECT_MODULE,
                EFFECT_NAME,
                |mut caller: Caller<'_, HostState<P>>,
                 slot: i32,
                 operation: i64,
                 resource_tag: i64,
                 pointer: i32,
                 length: i32|
                 -> i32 {
                    effect_call(&mut caller, slot, operation, resource_tag, pointer, length)
                },
            )
            .map_err(|_| ConfinedRefusal::ArtifactInvalid)?;
    }
    let instance = linker
        .instantiate_and_start(&mut store, &module)
        .map_err(|_| ConfinedRefusal::ExecutionFailed)?;
    let memory = instance
        .get_memory(&store, "memory")
        .ok_or(ConfinedRefusal::ArtifactInvalid)?;
    memory
        .write(&mut store, INPUT_OFFSET, input)
        .map_err(|_| ConfinedRefusal::MalformedGuestMemory)?;
    let run = instance
        .get_typed_func::<i32, i32>(&store, "conduit_run")
        .map_err(|_| ConfinedRefusal::ArtifactInvalid)?;
    let output_len = run.call(&mut store, input.len() as i32).map_err(|error| {
        if error.as_trap_code() == Some(TrapCode::OutOfFuel) {
            ConfinedRefusal::FuelExhausted
        } else {
            store.data().refusal.clone().unwrap_or_else(|| {
                let _ = error;
                ConfinedRefusal::ExecutionFailed
            })
        }
    })?;
    if let Some(refusal) = store.data().refusal.clone() {
        return Err(refusal);
    }
    let output_len = usize::try_from(output_len).map_err(|_| ConfinedRefusal::OutputLimit)?;
    if output_len > artifact.limits.maximum_output_bytes {
        return Err(ConfinedRefusal::OutputLimit);
    }
    let mut output = vec![0; output_len];
    memory
        .read(&store, OUTPUT_OFFSET, &mut output)
        .map_err(|_| ConfinedRefusal::MalformedGuestMemory)?;
    let remaining = store
        .get_fuel()
        .map_err(|_| ConfinedRefusal::ExecutionFailed)?;
    Ok(ConfinedExecution {
        output,
        fuel_consumed: artifact.limits.maximum_fuel - remaining,
        host_calls: store.data().host_calls,
        memory_bytes: memory.data_size(&store),
        imports: artifact.imports.clone(),
        realization_class: artifact.realization_class,
    })
}

struct HostState<P> {
    limits: StoreLimits,
    authority: Option<ConfinedAuthority>,
    provider: P,
    host_calls: u32,
    maximum_host_calls: u32,
    refusal: Option<ConfinedRefusal>,
}

fn effect_call<P: ConfinedBaseProvider>(
    caller: &mut Caller<'_, HostState<P>>,
    slot: i32,
    operation: i64,
    resource_tag: i64,
    pointer: i32,
    length: i32,
) -> i32 {
    let fail = |caller: &mut Caller<'_, HostState<P>>, refusal| {
        caller.data_mut().refusal = Some(refusal);
        -1
    };
    let Some(next_calls) = caller.data().host_calls.checked_add(1) else {
        return fail(caller, ConfinedRefusal::HostCallLimit);
    };
    if next_calls > caller.data().maximum_host_calls {
        return fail(caller, ConfinedRefusal::HostCallLimit);
    }
    caller.data_mut().host_calls = next_calls;
    let Some(memory) = caller.get_export("memory").and_then(Extern::into_memory) else {
        return fail(caller, ConfinedRefusal::MalformedGuestMemory);
    };
    let (pointer, length) = match (usize::try_from(pointer), usize::try_from(length)) {
        (Ok(pointer), Ok(length)) if length <= 4096 => (pointer, length),
        _ => return fail(caller, ConfinedRefusal::MalformedGuestMemory),
    };
    let mut request = [0; 4096];
    if memory
        .read(&*caller, pointer, &mut request[..length])
        .is_err()
    {
        return fail(caller, ConfinedRefusal::MalformedGuestMemory);
    }
    let lease = {
        let Some(authority) = caller.data_mut().authority.as_mut() else {
            return fail(caller, ConfinedRefusal::MissingAuthority);
        };
        if u32::try_from(slot).ok() != Some(authority.slot) {
            return fail(caller, ConfinedRefusal::ForgedSlot);
        }
        if u32::try_from(operation).ok() != Some(operation_tag(&authority.claim)) {
            return fail(caller, ConfinedRefusal::WrongOperation);
        }
        if resource_tag as u64 != authority.resource_tag {
            return fail(caller, ConfinedRefusal::WrongResource);
        }
        authority.claim.parameter_bytes = length as u32;
        match authority
            .table
            .authorize(&authority.handle, &authority.claim)
        {
            Ok(lease) => lease,
            Err(refusal) => return fail(caller, ConfinedRefusal::Capability(refusal)),
        }
    };
    match caller.data_mut().provider.call(&request[..length]) {
        Ok(result_bytes) => match caller
            .data_mut()
            .authority
            .as_mut()
            .expect("authority existed for lease")
            .table
            .complete(lease, result_bytes)
        {
            Ok(()) => 0,
            Err(refusal) => fail(caller, ConfinedRefusal::Capability(refusal)),
        },
        Err(refusal) => fail(caller, ConfinedRefusal::BaseRefused(refusal)),
    }
}

pub fn operation_tag(claim: &BaseOperationClaim) -> u32 {
    let digest = Sha256::digest(claim.operation_contract_id.as_str().as_bytes());
    u32::from_le_bytes(digest[..4].try_into().unwrap())
}

fn engine(limits: ConfinedLimits) -> Result<Engine, ConfinedRefusal> {
    validate_limits(limits)?;
    let mut config = Config::default();
    config.consume_fuel(true);
    Ok(Engine::new(&config))
}

fn validate_limits(limits: ConfinedLimits) -> Result<(), ConfinedRefusal> {
    if limits.maximum_artifact_bytes == 0
        || !(64 * 1024..=1024 * 1024).contains(&limits.maximum_memory_bytes)
        || limits.maximum_input_bytes == 0
        || limits.maximum_output_bytes == 0
        || OUTPUT_OFFSET + limits.maximum_output_bytes > limits.maximum_memory_bytes
        || limits.maximum_fuel == 0
        || limits.maximum_host_calls > 16
    {
        return Err(ConfinedRefusal::MemoryLimit);
    }
    Ok(())
}
