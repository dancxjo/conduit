//! Fabrication material for a single immutable checked Native value.
//! Host adapters choose facts; this module checks their Source-owned meaning.
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};

pub struct LinkedSourceImage {
    schema: Vec<u8>,
    value: Vec<u8>,
    maximum_bytes: u32,
}
impl LinkedSourceImage {
    /// Preserve the exact checked Type, including primitive refinements and
    /// invariants. This creates no Kind, offer, physical custody or Start grant.
    pub fn prepare(
        native: &conduit_plot::CheckedNativeType,
        value: &StructuredInfoValue,
    ) -> Result<Self, String> {
        if value.value_type() != &native.value_type {
            return Err("linked fact differs from checked Native Type".into());
        }
        // The runtime producer uses structured framing. Raw primitive ports
        // have a different ABI and must use their independently selected Back.
        if matches!(value.shape(), StructuredInfoValueShape::Leaf(_)) {
            return Err("linked fact requires a structured output".into());
        }
        conduit_plot::rust_binding::validate_native_contracts(value, &native.value_contracts)
            .and_then(|()| {
                conduit_plot::rust_binding::validate_native_invariants(value, &native.invariants)
            })
            .map_err(|e| format!("linked fact violates checked Native laws: {e:?}"))?;
        let maximum_bytes =
            conduit_plot::maximum_prepared_canonical_value_bytes(&native.value_type)
                .map_err(|e| format!("linked fact Type has no finite canonical extent: {e:?}"))?;
        let schema = native
            .value_type
            .canonical_bytes()
            .map_err(|e| format!("linked fact schema: {e:?}"))?;
        let value = value
            .canonical_bytes()
            .map_err(|e| format!("linked fact value: {e:?}"))?;
        if value.len() > maximum_bytes as usize {
            return Err("linked fact exceeds its original Type extent".into());
        }
        Ok(Self {
            schema,
            value,
            maximum_bytes,
        })
    }
    pub fn schema(&self) -> &[u8] {
        &self.schema
    }
    pub fn value(&self) -> &[u8] {
        &self.value
    }
    pub fn maximum_bytes(&self) -> u32 {
        self.maximum_bytes
    }
    /// Emit borrowed flash material for the common software factory. Selection,
    /// original Host configuration identity and funding remain with the caller.
    pub fn rust_definition(&self) -> String {
        format!("conduit_mcu_host_runtime::execution_factory::SoftwareDefinition::LinkedSourceStatic{{schema:&{:?},value:&{:?},maximum_bytes:{}}}", self.schema, self.value, self.maximum_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{StructuredFieldValue, StructuredInfoTypeShape};
    #[test]
    fn linked_image_keeps_checked_refinements_and_emits_real_factory_definition() {
        let checked = conduit_plot::check_syntax_document(
            &conduit_plot::parse_syntax_document("type Settings = {\n address: U8 in 8..=119\n}\n"),
            &conduit_plot::StartupCatalog::new(),
        )
        .unwrap();
        let native = &checked.native_types[0];
        let StructuredInfoTypeShape::Record { fields, .. } = native.value_type.shape() else {
            panic!("record");
        };
        let value = |address| {
            StructuredInfoValue::record(
                native.value_type.clone(),
                vec![StructuredFieldValue::new(
                    "address",
                    StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![address])
                        .unwrap(),
                )
                .unwrap()],
            )
            .unwrap()
        };
        assert!(LinkedSourceImage::prepare(native, &value(7)).is_err());
        assert!(LinkedSourceImage::prepare(native, &value(120)).is_err());
        let image = LinkedSourceImage::prepare(native, &value(118)).unwrap();
        let generated = format!("pub fn definition()->conduit_mcu_host_runtime::execution_factory::SoftwareDefinition<'static>{{ {} }}", image.rust_definition());
        crate::compiled_rust::verify_with_libraries(
            &generated,
            r#"
fn main() {
    let conduit_mcu_host_runtime::execution_factory::SoftwareDefinition::LinkedSourceStatic { schema, value, maximum_bytes } = definition() else { panic!("linked source"); };
    assert!(value.len() <= maximum_bytes);
    conduit_mcu_host_runtime::execution_software::SoftwareBack::linked_source_static(schema, value, maximum_bytes).unwrap();
}
"#,
            "linked-source-definition",
            &["conduit_mcu_host_runtime"],
        );
    }
}
