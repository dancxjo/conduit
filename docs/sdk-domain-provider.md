# Domain Provider SDK Guide

This guide describes how domain authors (such as Tongues, Netherwick, or custom sensor/robotics integrations) create domain-specific `TypeContract` value definitions, `NodeContract` boundaries, and host `Implementation` handlers without reading or modifying `conduit-core` internals.

---

## Architectural Separation

Conduit maintains a strict boundary between domain semantics, host execution, and source presentation:

| Identity | Location | Description |
|---|---|---|
| **TypeContract** | Domain / Crate | Value type meaning, compatibility, and canonical representation schema. |
| **NodeContract** | Domain / Catalog | Concrete node boundary specifying ports, config requirements, and resource shape. |
| **Implementation** | Provider / Host | Target-executable handler logic selected by host resolution (e.g., Linux, RP2040, WASM). |
| **ExecutionPlan** | Conduit Runtime | Lowered, immutable execution plan with frozen budgets and deterministic evidence tracing. |

---

## Step 1: Define Custom Type Contracts

A domain provider defines domain values by creating `TypeContract` instances in its own crate:

```rust
use conduit_core::{Id, SemanticHash, TypeContract, TypeContractRef};

pub const AUDIO_PCM_TYPE: TypeContractRef<'static> = &TypeContract {
    id: Id("speech/audio-pcm"),
    schema_version: 1,
    semantic_hash: SemanticHash::from_bytes([
        0x73, 0x70, 0x65, 0x65, 0x63, 0x68, 0x2d, 0x61, 0x75, 0x64, 0x69, 0x6f, 0x2d, 0x70,
        0x63, 0x6d, 0x2d, 0x76, 0x31, 0x2d, 0x73, 0x65, 0x6d, 0x61, 0x6e, 0x74, 0x69, 0x63,
        0x30, 0x30, 0x30, 0x31,
    ]),
};
```

---

## Step 2: Define Node Contracts

Define the public interface of domain nodes:

```rust
use conduit_core::{
    Cardinality, DeliverySemantics, Id, NodeContract, PortContract,
    SensitivityLevel, TerminalSemantics,
};

pub const ASR_RECOGNIZER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("tongues/asr-recognizer"),
    config: &[],
    inputs: &[PortContract {
        name: Id("audio"),
        value_type: AUDIO_PCM_TYPE,
        cardinality: Cardinality::Exact(1),
        delivery: DeliverySemantics::AtLeastOnce,
        terminal: TerminalSemantics::Never,
        sensitivity: SensitivityLevel::Public,
    }],
    outputs: &[PortContract {
        name: Id("transcript"),
        value_type: AUDIO_PCM_TYPE,
        cardinality: Cardinality::Exact(1),
        delivery: DeliverySemantics::AtLeastOnce,
        terminal: TerminalSemantics::Never,
        sensitivity: SensitivityLevel::Public,
    }],
};
```

---

## Step 3: Register Provider Nodes & Implementations

Domain providers register node contracts and host handlers in the runtime registry:

```rust
use conduit_runtime::{Registry, Value};

pub fn register_domain_provider(registry: &mut Registry) {
    registry.register_node(
        &ASR_RECOGNIZER_CONTRACT,
        || Box::new(AsrHandler::default()),
        |_node| Ok(()),
    );
}

#[derive(Default)]
struct AsrHandler;

impl conduit_runtime::Handler for AsrHandler {
    fn run(
        &mut self,
        _node: &conduit_runtime::Node,
        inputs: &[Value],
        _io: &mut conduit_runtime::RunIo<'_>,
    ) -> Result<Vec<Value>, conduit_runtime::RuntimeError> {
        // Process domain input values deterministically
        Ok(inputs.to_vec())
    }
}
```

---

## Recipe: Checking Custom Domain Panels

Domain users author `.panel` source files referencing your domain node:

```panel
panel 1

node audio_source : conduit.std/literal {
    value = "sample_audio_pcm_payload"
}

node asr : tongues/asr-recognizer
node output_log : conduit.std/log

cord audio_source.out -> asr.audio {
    capacity = 8
    max_value_bytes = 65536
    max_queued_bytes = 524288
    low_watermark = 2
    high_watermark = 8
    pressure = block
}

cord asr.transcript -> output_log.in {
    capacity = 8
    max_value_bytes = 65536
    max_queued_bytes = 524288
    low_watermark = 2
    high_watermark = 8
    pressure = block
}
```

This source is **illustrative/unavailable** until the domain provider,
implementation artifacts, capabilities, and grants named by an exact plan are
installed on the selected host. Syntax and inspection remain useful, but the
presence of a provider contract is not execution authority:

```sh
# Validate syntax and check lowerings
conduct check my_asr_pipeline.panel

# Inspect lowering identities and execution plans
conduct explain my_asr_pipeline.panel
```

---

## Rules for Domain Authors

1. **No Core Mutating**: Never add domain concepts (speech, vision, robotics, AI models) to `conduit-core`. Core remains `#![no_std]` and allocator-free.
2. **Explicit Authority**: Specify explicit capabilities (e.g. `wifi-network`, `http-client`, `gpio-pin`) instead of requesting ambient host authority.
3. **Bounded Queues**: Cord capacities, value byte ceilings, and queued byte limits must be explicitly specified in `.panel` source or plan definitions.
4. **Deterministic Evidence**: All execution steps emit structured, immutable execution evidence records.
