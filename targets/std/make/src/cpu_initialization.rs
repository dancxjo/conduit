//! Explicit native 64-bit CPU profile intent. Static checking validates the
//! requested target; fabrication does not execute or emulate its CPU provider.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    ImplementationMetadata, ImplementationOffer, KindConfigurationField, KindConfigurationRule,
    PrerequisiteNode,
};
pub const CPU_BASE_KIND: &str = "compute/cpu";
pub const CPU_IMPLEMENTATION: &str = "hosted/cpu-64@1";
pub fn cpu_initialization_contract() -> BaseInitializationContract {
    BaseInitializationContract {
        fields: vec![
            KindConfigurationField {
                key: "pointer_bits".into(),
                default_value: ConfigurationValue::U64(64),
                rule: KindConfigurationRule::U64Range {
                    minimum: 64,
                    maximum: 64,
                },
            },
            KindConfigurationField {
                key: "memory_pool".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::DataLabel { maximum: 256 },
            },
            KindConfigurationField {
                key: "memory_bytes".into(),
                default_value: ConfigurationValue::U64(1),
                rule: KindConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: u32::MAX as u64,
                },
            },
        ],
    }
}
pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: CPU_BASE_KIND.into(),
        implementation_id: CPU_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec!["std/x86_64/*".into(), "std/aarch64/*".into()],
        prerequisites: vec![],
        build_feature: None,
    }
}
pub(super) fn operation_metadata(offer: &conduit_core::CapabilityOffer) -> ImplementationMetadata {
    ImplementationMetadata {
        kind: offer.kind_id.as_str().into(),
        contract_revision: offer.kind_contract_revision.as_str().into(),
        targets: vec!["std/x86_64/*".into(), "std/aarch64/*".into()],
        prerequisites: vec![
            PrerequisiteNode::Base(CPU_BASE_KIND.into()),
            PrerequisiteNode::Resource(conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS.into()),
        ]
        .into_iter()
        .chain(
            offer
                .host_calls
                .iter()
                .map(|call| PrerequisiteNode::HostCall(call.contract_id.as_str().into())),
        )
        .collect(),
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCpuInitialization {
    pub configuration_id: String,
    pub memory_pool: String,
    pub memory_bytes: u32,
}
impl CheckedCpuInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let configuration = checked.configuration();
        if configuration.target.os.as_deref() != Some("linux")
            || !matches!(
                configuration.target.architecture.as_str(),
                "x86_64" | "aarch64"
            )
        {
            return Err("selected CPU profile requires a supported 64-bit Linux target".into());
        }
        let mut bases = configuration
            .bases
            .iter()
            .filter(|base| base.kind == CPU_BASE_KIND);
        let base = bases.next().ok_or("no selected CPU Base")?;
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, implementation)| {
                    kind == CPU_BASE_KIND && implementation == CPU_IMPLEMENTATION
                })
        {
            return Err("selected CPU requires one exact native implementation".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("selected CPU requires explicit finite initialization")?
            .checked_values(&cpu_initialization_contract())?;
        let memory_pool = match values.get("memory_pool") {
            Some(ConfigurationValue::Text(value)) => value.clone(),
            _ => return Err("selected CPU memory pool missing".into()),
        };
        let memory_bytes = match values.get("memory_bytes") {
            Some(ConfigurationValue::U64(value)) => {
                u32::try_from(*value).map_err(|e| e.to_string())?
            }
            _ => return Err("selected CPU memory capacity missing".into()),
        };
        let budgets = configuration
            .resources
            .iter()
            .filter(|budget| budget.id == memory_pool)
            .collect::<Vec<_>>();
        if budgets.len() != 1
            || budgets[0].class != conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS
            || budgets[0].slots != memory_bytes
            || budgets[0].bytes != u64::from(memory_bytes)
            || u64::from(memory_bytes) > configuration.limits.heap_arena_bytes
        {
            return Err("selected CPU requires its exact finite byte-counted memory pool within IMAGE heap bounds".into());
        }
        Ok(Self {
            configuration_id: checked.configuration_id().into(),
            memory_pool,
            memory_bytes,
        })
    }
}
