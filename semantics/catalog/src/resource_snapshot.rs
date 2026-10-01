//! Exact-generation publication and reading of bounded JSON snapshot content.
use crate::{KindConfigurationField, KindConfigurationRule, StandardKindContract};
#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use conduit_core::{
    kind_id, ConfigurationValue, FrontStartupParameter, Kind, KindIdentity, KindSemanticLaw,
    RESOURCE_REFERENCE_INFO_ID,
};
pub const SNAPSHOT_PUBLISH_KIND: &str = "resource/publish-json-snapshot";
pub const SNAPSHOT_READ_KIND: &str = "resource/read-json-snapshot";
pub const SNAPSHOT_REVISION: &str = "conduit.resource/json-snapshot@1";

pub fn resource_snapshot_contract(publish: bool) -> StandardKindContract {
    let mut contract = crate::json_decode_contract();
    contract.kind_id = kind_id(if publish {
        SNAPSHOT_PUBLISH_KIND
    } else {
        SNAPSHOT_READ_KIND
    });
    contract.plain_name = if publish {
        "Publish JSON snapshot"
    } else {
        "Read JSON snapshot"
    }
    .into();
    contract.summary =
        "Access one exact bounded Resource generation through separately admitted authority."
            .into();
    contract.inputs[0].value_kind = kind_id(if publish {
        conduit_web::JSON_TEXT_INFO_ID
    } else {
        RESOURCE_REFERENCE_INFO_ID
    });
    contract.outputs[0].value_kind = kind_id(if publish {
        RESOURCE_REFERENCE_INFO_ID
    } else {
        conduit_web::JSON_TEXT_INFO_ID
    });
    contract.configuration.push(KindConfigurationField {
        key: "reference".into(),
        default_value: ConfigurationValue::Text("".into()),
        rule: KindConfigurationRule::TextBytes { maximum: 1024 },
    });
    contract.limits.max_active_instances = 1;
    contract.limits.max_queue_items = 1;
    contract
}

pub fn resource_snapshot_semantic_contract(publish: bool) -> Kind {
    let contract = resource_snapshot_contract(publish);
    Kind {
        startup_parameters: alloc::vec![FrontStartupParameter {
            name: "reference".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        }],
        shorthand: None,
        kind_id: contract.kind_id,
        kind_contract_revision: KindIdentity::from(SNAPSHOT_REVISION),
        inputs: contract.inputs,
        outputs: contract.outputs,
        configuration: contract.configuration,
        semantic_laws: alloc::vec![KindSemanticLaw::Terminal(contract.terminal_behavior)],
        limits: contract.limits,
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_resource_snapshot_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    for publish in [true, false] {
        let contract = resource_snapshot_semantic_contract(publish);
        startup.insert(conduit_form::KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: alloc::vec![conduit_form::StartupParameterSignature {
                name: "reference".into(),
                value_type: "Text".into(),
                default: None,
            }],
        })?;
        profile
            .insert_kind(contract)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
