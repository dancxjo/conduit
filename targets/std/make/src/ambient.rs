//! Checked ambient equipment intent. These fields grant no network authority.
use conduit_host_make::{
    BaseInitializationContract, CheckedHostConfiguration, ConfigurationValue,
    ImplementationMetadata, ImplementationOffer, KindConfigurationField, KindConfigurationRule,
    PrerequisiteNode,
};
use conduit_std_offers::*;
use std::net::SocketAddr;
pub fn ambient_initialization_contract() -> BaseInitializationContract {
    let text = |key: &str, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::Text(String::new()),
        rule: KindConfigurationRule::DataLabel { maximum },
    };
    let count = |key: &str, minimum, maximum| KindConfigurationField {
        key: key.into(),
        default_value: ConfigurationValue::U64(minimum),
        rule: KindConfigurationRule::U64Range { minimum, maximum },
    };
    BaseInitializationContract {
        fields: vec![
            text("timezone", 64),
            count("maximum_updates", 1, 4096),
            text("weather_endpoint", 64),
            text("source_pool", 256),
            text("memory_pool", 256),
            count("memory_bytes", 1, u32::MAX as u64),
            count(
                "preparation_bytes",
                AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES as u64,
                AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES as u64,
            ),
            count(
                "retained_bytes",
                1,
                AMBIENT_SOURCE_PROVIDER_PREPARATION_BYTES as u64,
            ),
        ],
    }
}
pub fn ambient_resident_initialization_contract() -> BaseInitializationContract {
    let mut contract = ambient_initialization_contract();
    contract.fields.retain(|f| f.key != "maximum_updates");
    contract
}
pub(super) fn resident_offer() -> ImplementationOffer {
    let mut offer = offer();
    offer.base_kind = AMBIENT_RESIDENT_SOURCE_BASE_KIND.into();
    offer.implementation_id = AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION.into();
    offer
}
pub(super) fn resident_metadata() -> ImplementationMetadata {
    let mut metadata = metadata();
    metadata.kind = "ambient/resident-observation".into();
    metadata.contract_revision = "ambient/resident-observation@1".into();
    metadata.prerequisites = vec![
        PrerequisiteNode::HostCall(AMBIENT_RESIDENT_SOURCE_CALL.into()),
        PrerequisiteNode::Resource(AMBIENT_RESIDENT_SOURCE_RESOURCE.into()),
    ];
    metadata
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmbientObservationLifetime {
    Finite(u32),
    Resident,
}

pub(super) fn offer() -> ImplementationOffer {
    ImplementationOffer {
        base_kind: AMBIENT_SOURCE_BASE_KIND.into(),
        implementation_id: AMBIENT_SOURCE_IMPLEMENTATION.into(),
        implementation_revision: 1,
        target_patterns: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: vec![],
        build_feature: None,
    }
}
pub(super) fn metadata() -> ImplementationMetadata {
    ImplementationMetadata {
        kind: "ambient/home-observation".into(),
        contract_revision: "ambient/home-observation@1".into(),
        targets: vec![crate::HOSTED_TARGET_ID.into()],
        prerequisites: vec![
            PrerequisiteNode::HostCall(AMBIENT_SOURCE_CALL.into()),
            PrerequisiteNode::Resource(AMBIENT_SOURCE_RESOURCE.into()),
        ],
    }
}
pub(super) fn resident_resource_dependency() -> (PrerequisiteNode, Vec<PrerequisiteNode>) {
    (
        PrerequisiteNode::Resource(AMBIENT_RESIDENT_SOURCE_RESOURCE.into()),
        vec![PrerequisiteNode::Base(
            AMBIENT_RESIDENT_SOURCE_BASE_KIND.into(),
        )],
    )
}
pub(super) fn resource_dependency() -> (PrerequisiteNode, Vec<PrerequisiteNode>) {
    (
        PrerequisiteNode::Resource(AMBIENT_SOURCE_RESOURCE.into()),
        vec![PrerequisiteNode::Base(AMBIENT_SOURCE_BASE_KIND.into())],
    )
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedAmbientInitialization {
    pub configuration_id: String,
    pub timezone: String,
    pub lifetime: AmbientObservationLifetime,
    pub weather_endpoint: SocketAddr,
    pub source_pool: String,
    pub memory_pool: String,
    pub memory_bytes: u32,
    /// Provider-only ceilings; the existing worker is charged in addition.
    pub preparation_bytes: u32,
    pub retained_bytes: u32,
}
impl CheckedAmbientInitialization {
    pub fn from_configuration(checked: &CheckedHostConfiguration) -> Result<Self, String> {
        let config = checked.configuration();
        if config.target.os.as_deref() != Some("linux") || config.target.architecture != "x86_64" {
            return Err("ambient worker requires the supported 64-bit Linux target".into());
        }
        let mut bases = config.bases.iter().filter(|b| {
            b.kind == AMBIENT_SOURCE_BASE_KIND || b.kind == AMBIENT_RESIDENT_SOURCE_BASE_KIND
        });
        let base = bases.next().ok_or("no selected ambient observation Base")?;
        let resident = base.kind == AMBIENT_RESIDENT_SOURCE_BASE_KIND;
        let implementation = if resident {
            AMBIENT_RESIDENT_SOURCE_IMPLEMENTATION
        } else {
            AMBIENT_SOURCE_IMPLEMENTATION
        };
        if bases.next().is_some()
            || !checked
                .resolved_bases()
                .iter()
                .any(|(kind, selected)| kind == &base.kind && selected == implementation)
        {
            return Err("ambient observation requires one exact selected worker".into());
        }
        let values = base
            .initialization
            .as_ref()
            .ok_or("missing ambient initialization")?
            .checked_values(&if resident {
                ambient_resident_initialization_contract()
            } else {
                ambient_initialization_contract()
            })?;
        let text = |key: &str| match values.get(key) {
            Some(ConfigurationValue::Text(v)) => Ok(v.clone()),
            _ => Err("ambient text field has wrong type".to_owned()),
        };
        let count = |key: &str| match values.get(key) {
            Some(ConfigurationValue::U64(v)) => {
                u32::try_from(*v).map_err(|_| "ambient count exceeds local bound".to_owned())
            }
            _ => Err("ambient count field has wrong type".to_owned()),
        };
        let result = Self {
            configuration_id: checked.configuration_id().into(),
            timezone: text("timezone")?,
            lifetime: if resident {
                AmbientObservationLifetime::Resident
            } else {
                AmbientObservationLifetime::Finite(count("maximum_updates")?)
            },
            weather_endpoint: text("weather_endpoint")?
                .parse()
                .map_err(|_| "ambient endpoint requires an explicit IP socket address")?,
            source_pool: text("source_pool")?,
            memory_pool: text("memory_pool")?,
            memory_bytes: count("memory_bytes")?,
            preparation_bytes: count("preparation_bytes")?,
            retained_bytes: count("retained_bytes")?,
        };
        if result.timezone.is_empty()
            || result.timezone.chars().any(char::is_control)
            || result.weather_endpoint.port() == 0
            || result.source_pool == result.memory_pool
            || result.retained_bytes > result.preparation_bytes
        {
            return Err(
                "ambient initialization identity, endpoint or provider budget invalid".into(),
            );
        }
        let required = result
            .preparation_bytes
            .checked_add(AMBIENT_SOURCE_WORKER_BACKING_BYTES)
            .ok_or("ambient preparation overflow")?;
        if result.memory_bytes < required
            || u64::from(result.memory_bytes) > config.limits.heap_arena_bytes
        {
            return Err(
                "ambient pool omits complete provider/worker preparation or exceeds IMAGE heap"
                    .into(),
            );
        }
        for (id, class, slots, bytes) in [
            (
                &result.source_pool,
                if resident {
                    AMBIENT_RESIDENT_SOURCE_RESOURCE
                } else {
                    AMBIENT_SOURCE_RESOURCE
                },
                1,
                8192u64,
            ),
            (
                &result.memory_pool,
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                result.memory_bytes,
                u64::from(result.memory_bytes),
            ),
        ] {
            let mut budgets = config.resources.iter().filter(|b| b.id == *id);
            let budget = budgets
                .next()
                .ok_or("ambient explicit resource pool missing")?;
            if budgets.next().is_some()
                || budget.class != class
                || budget.slots != slots
                || budget.bytes != bytes
            {
                return Err(
                    "ambient resource pool differs from its exact finite initialization".into(),
                );
            }
        }
        Ok(result)
    }
}
