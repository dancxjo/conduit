//! Build-Host custody of checked immutable values. Runtime gets borrowed bytes,
//! never an ambient configuration lookup, decoded Kind or preparation authority.
use crate::{composite::ValueReference, linked_source::LinkedSourceImage};
use conduit_core as c;
use conduit_plot as p;
use sha2::{Digest, Sha256};
use std::fmt::Write;

pub struct InstalledValueInput {
    pub key: String,
    pub type_name: String,
    pub maximum_bytes: u32,
    pub image: LinkedSourceImage,
}
#[derive(Clone)]
struct Binding {
    key: String,
    kind: c::Kind,
    image: LinkedSourceImage,
    maximum_bytes: u32,
    digest: [u8; 32],
}
#[derive(Clone, Default)]
pub struct InstalledValueBindings {
    configuration_id: Option<String>,
    source_document_id: Option<c::SourceDocumentId>,
    values: Vec<Binding>,
}
pub(crate) struct Selection {
    pub kind: c::Kind,
    pub index: usize,
    pub digest: [u8; 32],
}
impl InstalledValueBindings {
    pub fn from_checked(
        host: &conduit_host_make::CheckedHostConfiguration,
        source: &p::CheckedSyntaxDocument,
        values: Vec<InstalledValueInput>,
    ) -> Result<Self, String> {
        Self::prepare(host.configuration_id(), source, values)
    }
    fn prepare(
        configuration_id: &str,
        source: &p::CheckedSyntaxDocument,
        values: Vec<InstalledValueInput>,
    ) -> Result<Self, String> {
        if values.len() > 16 || configuration_id.is_empty() {
            return Err("installed value binding extent/configuration".into());
        }
        let mut result = Self {
            configuration_id: Some(configuration_id.into()),
            source_document_id: Some(source.source_document_id.clone()),
            values: Vec::new(),
        };
        for value in values {
            let configuration = [c::ConfigurationEntry {
                key: "key".into(),
                value: c::ConfigurationValue::Text(value.key.clone()),
            }];
            conduit_semantic_catalog::installed_value_once_key(&configuration)?;
            if result.values.iter().any(|entry| entry.key == value.key) {
                return Err("duplicate installed value key".into());
            }
            let mut types = source
                .native_types
                .iter()
                .filter(|ty| ty.name == value.type_name);
            let native = types.next().ok_or("installed value Native Type missing")?;
            if types.next().is_some() {
                return Err("ambiguous installed value Native Type".into());
            }
            let decoded = c::StructuredInfoValue::from_canonical_bytes(value.image.value())
                .map_err(|e| format!("installed value canonical frame: {e:?}"))?;
            let validated = LinkedSourceImage::prepare(native, &decoded)?;
            if validated.schema() != value.image.schema()
                || validated.value() != value.image.value()
            {
                return Err("installed image differs from original checked Source".into());
            }
            let contract = contract(&native.value_type, value.maximum_bytes)?;
            if value.image.value().len() > value.maximum_bytes as usize {
                return Err("installed value exceeds selected original envelope".into());
            }
            let kind = conduit_semantic_catalog::installed_value_once_semantic_contract(
                &contract,
                &native.value_type,
            )?;
            let mut hash = Sha256::new();
            for bytes in [
                configuration_id.as_bytes(),
                value.key.as_bytes(),
                value.image.schema(),
                value.image.value(),
            ] {
                hash.update((bytes.len() as u64).to_le_bytes());
                hash.update(bytes);
            }
            result.values.push(Binding {
                key: value.key,
                kind,
                image: value.image,
                maximum_bytes: value.maximum_bytes,
                digest: hash.finalize().into(),
            });
        }
        Ok(result)
    }
    pub(crate) fn identity(&self) -> [u8; 32] {
        if self.values.is_empty() {
            return [0; 32];
        }
        let mut hash = Sha256::new();
        for value in &self.values {
            hash.update(value.digest);
        }
        hash.finalize().into()
    }
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    pub(crate) fn validate_source(&self, source: &p::CheckedSyntaxDocument) -> Result<(), String> {
        if !self.values.is_empty()
            && self.source_document_id.as_ref() != Some(&source.source_document_id)
        {
            return Err("installed values belong to another checked Source".into());
        }
        Ok(())
    }
    pub fn validate_configuration(
        &self,
        host: &conduit_host_make::CheckedHostConfiguration,
    ) -> Result<(), String> {
        if !self.values.is_empty()
            && self.configuration_id.as_deref() != Some(host.configuration_id())
        {
            return Err("installed values belong to another Host configuration".into());
        }
        Ok(())
    }
    pub(crate) fn select(&self, gear: &p::CheckedGear) -> Result<Option<Selection>, String> {
        if !is_instance(gear.kind_id.as_str()) {
            return Ok(None);
        }
        let key = conduit_semantic_catalog::installed_value_once_key(&gear.configuration)?;
        let (index, value) = self
            .values
            .iter()
            .enumerate()
            .find(|(_, value)| value.key == key)
            .ok_or("installed value key unavailable")?;
        if gear.kind_id != value.kind.kind_id {
            return Err("installed value exact Type/envelope differs".into());
        }
        Ok(Some(Selection {
            kind: value.kind.clone(),
            index,
            digest: value.digest,
        }))
    }
    pub(crate) fn emit_shared(&self) -> String {
        let mut code = String::from("pub mod installed_values{\n");
        for (index, value) in self.values.iter().enumerate() {
            writeln!(code,"pub static SCHEMA_{index}:&[u8]=&{:?};\npub static VALUE_{index}:&[u8]=&{:?};\npub const MAXIMUM_{index}:usize={};",value.image.schema(),value.image.value(),value.maximum_bytes).unwrap();
        }
        code.push_str("}\n");
        code
    }
}
fn contract(
    schema: &c::StructuredInfoType,
    maximum: u32,
) -> Result<c::CheckedValueContract, String> {
    if maximum == 0 || maximum > 4096 {
        return Err("MCU installed value exceeds existing4096-byte construction profile".into());
    }
    c::CheckedValueContract::new(
        schema
            .profile()
            .map_err(|e| format!("{e:?}"))?
            .value_kind()
            .clone(),
        maximum,
        vec![],
    )
    .map_err(|e| format!("{e:?}"))
}
pub(crate) fn is_instance(name: &str) -> bool {
    name.strip_prefix(conduit_semantic_catalog::INSTALLED_VALUE_ONCE_KIND)
        .is_some_and(|tail| tail.starts_with("/typed-"))
}
pub(crate) fn install(
    syntax: &p::SyntaxDocument,
    selections: &[ValueReference<'_>],
    startup: &mut p::StartupCatalog,
    profiles: &mut p::ProfileCatalog,
) -> Result<(), String> {
    if selections.is_empty() {
        return Ok(());
    }
    if selections.len() > 16 {
        return Err("installed value specialization count".into());
    }
    let mut declarations = syntax.clone();
    declarations
        .uses
        .retain(|import| !is_instance(&import.path));
    let types = p::prepare_source_types(&declarations, startup)
        .map_err(|e| format!("installed value Types: {e:?}"))?;
    let mut identities = std::collections::BTreeSet::new();
    for selection in selections {
        let schema = types
            .named_type(selection.type_name)
            .ok_or("unknown installed value Source Type")?;
        let contract = contract(schema, selection.maximum_bytes)?;
        let kind =
            conduit_semantic_catalog::installed_value_once_semantic_contract(&contract, schema)?;
        if !identities.insert(kind.kind_id) {
            return Err("duplicate installed value specialization".into());
        }
        conduit_semantic_catalog::install_installed_value_once_kind(
            &contract, schema, startup, profiles,
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "installed_values_tests.rs"]
mod tests;
