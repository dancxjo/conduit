//! Checked semantic type registration and retained refinement metadata.
use super::*;

impl StartupCatalog {
    /// Admit one owner-validated initial Form for a custom Info Kind.
    /// The literal is the exact source spelling, including quotes where used.
    /// The semantic owner must supply its decoder as `validate`; no raw bytes
    /// are admitted without passing it at catalog preparation time.
    pub fn insert_exact_initial_info(
        &mut self,
        value_kind: conduit_core::KindId,
        literal: impl Into<String>,
        bytes: Vec<u8>,
        validate: fn(&[u8]) -> bool,
    ) -> Result<(), String> {
        let literal = literal.into();
        if literal.is_empty()
            || literal.len() > 256
            || conduit_core::primitive_info_kind(value_kind.as_str()).is_some()
        {
            return Err("exact initial Info requires a nonempty literal and custom Kind".into());
        }
        if bytes.len() > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES || !validate(&bytes) {
            return Err("exact initial Info failed its owner's finite Form validation".into());
        }
        let key = (value_kind, literal);
        if self.exact_initial_info.contains_key(&key) {
            return Err("duplicate exact initial Info literal".into());
        }
        if self.exact_initial_info.len() >= 64
            || self
                .exact_initial_info
                .values()
                .map(Vec::len)
                .sum::<usize>()
                .saturating_add(bytes.len())
                > conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES
        {
            return Err("exact initial Info registry exceeds its finite bound".into());
        }
        self.exact_initial_info.insert(key, bytes);
        Ok(())
    }

    pub fn insert_structured_type(
        &mut self,
        name: impl Into<String>,
        value_type: conduit_core::StructuredInfoType,
    ) -> Result<(), String> {
        let name = name.into();
        if name.is_empty() {
            return Err("structured startup type name must not be empty".into());
        }
        if self.structured_types.contains_key(&name) || self.value_kind_aliases.contains_key(&name)
        {
            return Err(format!("duplicate structured startup type '{name}'"));
        }
        self.structured_types.insert(name, value_type);
        Ok(())
    }

    /// Register a shared owner schema idempotently. A different namesake or
    /// value-Kind alias remains an error; existing refinements are preserved.
    pub fn ensure_structured_type(
        &mut self,
        name: impl Into<String>,
        value_type: conduit_core::StructuredInfoType,
    ) -> Result<(), String> {
        let name = name.into();
        match self.structured_types.get(&name) {
            Some(existing) if existing == &value_type => Ok(()),
            Some(_) => Err(format!(
                "structured startup type '{name}' differs from its owner schema"
            )),
            None => self.insert_structured_type(name, value_type),
        }
    }

    /// Returns the authored semantic Type name for an exact structured value Kind.
    ///
    /// Presentation surfaces use this to keep the stable human-facing Type name
    /// while preserving the profile identity as the executable contract.
    pub fn structured_type_name(&self, value_kind: &conduit_core::KindId) -> Option<&str> {
        self.structured_types.iter().find_map(|(name, value_type)| {
            (value_type
                .profile()
                .is_ok_and(|profile| profile.value_kind() == value_kind))
            .then_some(name.as_str())
        })
    }

    pub(crate) fn insert_native_type(
        &mut self,
        name: impl Into<String>,
        value_type: conduit_core::StructuredInfoType,
        contracts: Vec<NativeTypeValueContract>,
        invariants: Vec<crate::PortableExpressionProgram>,
    ) -> Result<(), String> {
        let name = name.into();
        self.insert_structured_type(name.clone(), value_type)?;
        self.structured_type_contracts
            .insert(name.clone(), contracts);
        self.structured_type_invariants.insert(name, invariants);
        Ok(())
    }

    pub fn insert_value_kind_alias(
        &mut self,
        name: impl Into<String>,
        value_kind: conduit_core::KindId,
    ) -> Result<(), String> {
        let name = name.into();
        if name.is_empty() {
            return Err("startup value Kind alias must not be empty".into());
        }
        if self.value_kind_aliases.get(&name) == Some(&value_kind) {
            return Ok(());
        }
        if self.value_kind_aliases.contains_key(&name) || self.structured_types.contains_key(&name)
        {
            return Err(format!("duplicate startup value type '{name}'"));
        }
        self.value_kind_aliases.insert(name, value_kind);
        Ok(())
    }

    /// Register an imported checked Type with its exact refinement metadata.
    pub fn insert_checked_native_type(
        &mut self,
        name: impl Into<String>,
        ty: &CheckedNativeType,
    ) -> Result<(), String> {
        self.insert_native_type(
            name,
            ty.value_type.clone(),
            ty.value_contracts.clone(),
            ty.invariants.clone(),
        )
    }

    pub(crate) fn retained_native_types(&self) -> Vec<CheckedNativeType> {
        self.structured_types
            .iter()
            .filter_map(|(name, ty)| {
                let contracts = self.structured_type_contracts.get(name)?;
                Some(CheckedNativeType {
                    name: name.clone(),
                    identity: match ty.shape() {
                        conduit_core::StructuredInfoTypeShape::Nominal { schema, .. }
                        | conduit_core::StructuredInfoTypeShape::Record { schema, .. }
                        | conduit_core::StructuredInfoTypeShape::Variant { schema, .. } => {
                            schema.clone()
                        }
                        _ => return None,
                    },
                    value_type: ty.clone(),
                    value_contracts: contracts.clone(),
                    invariants: self
                        .structured_type_invariants
                        .get(name)
                        .cloned()
                        .unwrap_or_default(),
                })
            })
            .collect()
    }

    pub(crate) fn structured_type(&self, name: &str) -> Option<&conduit_core::StructuredInfoType> {
        self.structured_types.get(name)
    }

    pub(crate) fn structured_type_contracts(
        &self,
        name: &str,
    ) -> Option<&[NativeTypeValueContract]> {
        self.structured_type_contracts.get(name).map(Vec::as_slice)
    }

    pub(crate) fn structured_type_invariants(
        &self,
        name: &str,
    ) -> Option<&[crate::PortableExpressionProgram]> {
        self.structured_type_invariants.get(name).map(Vec::as_slice)
    }

    pub(crate) fn structured_types_by_value_kind(
        &self,
    ) -> Result<
        BTreeMap<conduit_core::KindId, conduit_core::StructuredInfoType>,
        conduit_core::StructuredInfoRefusal,
    > {
        let mut checked = BTreeMap::new();
        for value_type in self.structured_types.values() {
            let value_kind = value_type.profile()?.value_kind().clone();
            if checked
                .insert(value_kind.clone(), value_type.clone())
                .is_some_and(|prior| prior != *value_type)
            {
                return Err(conduit_core::StructuredInfoRefusal::WrongType);
            }
        }
        Ok(checked)
    }

    pub(crate) fn value_kind_alias(&self, name: &str) -> Option<&conduit_core::KindId> {
        self.value_kind_aliases.get(name)
    }
}
