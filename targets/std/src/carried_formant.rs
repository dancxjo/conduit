//! Installation of one explicitly supplied immutable formant projection.
//! This produces PCM in the execution kernel. Physical playback and parser
//! commitment remain separate admissions; this factory never accepts Text.
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    carried_formant_kernel::CarriedFormantBack,
    kernel::MAXIMUM_PCM_BYTES,
};
use std::sync::Arc;

pub const KIND: &str = "speech/carried-formant";
pub const REVISION: &str = "conduit.speech/carried-formant@1";
pub const IMPLEMENTATION: &str = "conduit-native/carried-formant@1";
pub const PROFILE: &str = "conduit-native/carried-s16le-8000-mono@1";
pub const CAPABILITY: &str = "native-carried-formant";
pub const SOURCE_VALUE_KIND: &str = "conduit.speech/checked-carrier-identity@1";

/// Owns the exact admitted artifact. Installation does not consult an ambient
/// source registry. Each registry admits one source for this implementation.
pub struct CarriedFormantFactory {
    implementation: ImplementationId,
    source: Arc<PreparedOfflinePcm>,
    clock: u64,
    admission_identity: [u8; 32],
}
impl CarriedFormantFactory {
    pub fn new(admitted: PreparedOfflinePcm, clock: u64) -> Result<Self, String> {
        if clock == 0 {
            return Err("zero sample clock identity".into());
        }
        let admission_identity = conduit_core::semantic_digest(
            "conduit.speech/offline-pcm-admission@1",
            &admitted
                .admission()
                .clone()
                .encode()
                .map_err(|e| format!("offline admission: {e:?}"))?,
        );
        Ok(Self {
            admission_identity,
            implementation: IMPLEMENTATION.into(),
            source: Arc::new(admitted),
            clock,
        })
    }
    pub fn source(&self) -> &FrozenCarriedFormant {
        self.source.source()
    }
    pub fn admission(&self) -> &conduit_speech::semantic::SpeechOfflinePcmAdmission {
        self.source.admission()
    }
    pub fn configuration(&self) -> Vec<ConfigurationEntry> {
        let mut configuration = vec![
            ConfigurationEntry {
                key: "offline-authoring".into(),
                value: ConfigurationValue::Text(hex(&self.admission_identity)),
            },
            ConfigurationEntry {
                key: "clock".into(),
                value: ConfigurationValue::U64(self.clock),
            },
            ConfigurationEntry {
                key: "source".into(),
                value: ConfigurationValue::Text(hex(self.source().source_identity())),
            },
            ConfigurationEntry {
                key: "projection".into(),
                value: ConfigurationValue::Text(hex(self.source().projection_identity())),
            },
        ];
        configuration.sort_by(|a, b| a.key.cmp(&b.key));
        configuration
    }
    pub fn contract(&self) -> Kind {
        Kind {
            kind_id: kind_id(KIND),
            kind_contract_revision: REVISION.into(),
            startup_parameters: vec![],
            shorthand: None,
            inputs: vec![PortDescriptor {
                port_id: port_id("source"),
                value_kind: kind_id(SOURCE_VALUE_KIND),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            }],
            outputs: vec![PortDescriptor {
                port_id: port_id("audio"),
                value_kind: kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
                direction: PortDirection::Output,
                temporal: PortTemporal::Flow { closes: true },
                abnormal_kind: None,
            }],
            configuration: self
                .configuration()
                .into_iter()
                .map(|entry| {
                    let rule = match &entry.value {
                        ConfigurationValue::U64(value) => KindConfigurationRule::U64Range {
                            minimum: *value,
                            maximum: *value,
                        },
                        ConfigurationValue::Text(value) => KindConfigurationRule::TextOneOf {
                            values: vec![value.clone()],
                        },
                        _ => unreachable!(),
                    };
                    KindConfigurationField {
                        key: entry.key,
                        default_value: entry.value,
                        rule,
                    }
                })
                .collect(),
            semantic_laws: vec![KindSemanticLaw::ValueContracts(vec![
                FrontValueContract {
                    location: FrontValueLocation::Input(port_id("source")),
                    contract: CheckedValueContract::new(kind_id(SOURCE_VALUE_KIND), 32, vec![])
                        .expect("finite identity"),
                },
                FrontValueContract {
                    location: FrontValueLocation::Output(port_id("audio")),
                    contract: CheckedValueContract::new(
                        kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
                        MAXIMUM_PCM_BYTES as u32,
                        vec![],
                    )
                    .expect("finite PCM"),
                },
            ])],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: MAXIMUM_PCM_BYTES as u32,
            },
        }
    }
    pub fn offer(&self) -> CapabilityOffer {
        BackOfferBuilder::new(
            self.contract(),
            Back {
                capability_id: CAPABILITY.into(),
                execution_profile_id: PROFILE.into(),
                implementation_id: self.implementation.clone(),
                artifact_id: format!(
                    "{IMPLEMENTATION}/{}/{}",
                    hex(self.source().projection_identity()),
                    hex(&self.admission_identity)
                )
                .into(),
                host_calls: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
            },
        )
        .build()
    }
    fn admitted(&self, placement: &PlannedGear) -> Result<(), String> {
        let offered = self.offer();
        if placement.kind_id != offered.kind_id
            || placement.kind_contract_revision != offered.kind_contract_revision
            || placement.execution_profile_id != offered.implementation.execution_profile_id
            || placement.capability_id != offered.capability_id
            || placement.implementation_id != offered.implementation.implementation_id
            || placement.artifact_id != offered.implementation.artifact_id
            || placement.realization_properties != offered.realization_properties
            || placement.inputs != offered.inputs
            || placement.outputs != offered.outputs
            || placement.limits != offered.limits
            || placement.semantic_contract != self.contract().semantic_contract()
            || placement.configuration != self.configuration()
            || !placement.host_calls.is_empty()
            || !placement.resources.is_empty()
            || !placement.authority.is_empty()
            || placement.base.is_some()
            || !placement.realization_characteristics.is_empty()
            || !placement.pool_references.is_empty()
            || !placement.terminal_transductions.is_empty()
        {
            return Err("carried formant exact placement identity/configuration differs".into());
        }
        Ok(())
    }
}
impl KernelOperationFactory for CarriedFormantFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation
    }
    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.admitted(placement)?;
        let blocks = self
            .source()
            .total_frames()
            .div_ceil(conduit_speech::MAXIMUM_BLOCK_FRAMES as u64);
        let signs = blocks
            .checked_mul(16)
            .and_then(|n| n.checked_add(16))
            .and_then(|n| u16::try_from(n).ok())
            .ok_or("Sign budget exceeds finite installed profile")?;
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: MAXIMUM_PCM_BYTES as u32,
            maximum_value_bytes: MAXIMUM_PCM_BYTES as u32,
            host_requests: 0,
            sign_items: signs,
        })
    }
    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<
        Box<
            dyn StepBack<{ conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>
                + Send,
        >,
        String,
    > {
        self.admitted(placement)?;
        Ok(Box::new(CarriedFormantBack::prepare(
            self.source.clone(),
            self.clock,
        )?))
    }
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
